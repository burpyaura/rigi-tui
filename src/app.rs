use crate::api::series::{Episode, EpisodeDetail, Series};
use crate::api::RigiClient;
use crate::player::PlayerHandle;
use crate::store::Store;
use std::sync::mpsc::Sender;

pub enum Msg {
    SendOtpDone(Result<Option<String>, String>),
    VerifyOtpDone(Result<String, String>),
    SeriesLoaded(Result<Vec<Series>, String>),
    EpisodesLoaded { series_id: String, result: Result<Vec<Episode>, String> },
    EpisodeLoaded { series_id: String, result: Result<EpisodeDetail, String> },
    MpvExited,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Phone,
    Otp,
    Series,
    Episodes,
    Playing,
}

pub struct App {
    pub client: RigiClient,
    pub store: Store,
    pub tx: Sender<Msg>,
    pub screen: Screen,

    pub dialing_code: String,
    pub phone_input: String,
    pub otp_input: String,
    pub otp_field_code: bool,
    pub otp_hash: String,

    pub busy: bool,
    pub status: String,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub spinner_frame: usize,

    pub series: Vec<Series>,
    pub series_page: u32,
    pub series_sel: usize,
    pub series_auto_loaded: bool,
    pub series_exhausted: bool,

    pub episodes: Vec<Episode>,
    pub episodes_page: u32,
    pub ep_sel: usize,
    pub episodes_exhausted: bool,
    pub current_series: Option<Series>,

    pub player: Option<PlayerHandle>,
    pub playing_title: String,
}

impl App {
    pub fn new(client: RigiClient, store: Store, tx: Sender<Msg>) -> Self {
        let creds = store.load();
        let has_token = creds.token.is_some();
        if let Some(t) = creds.token {
            client.set_token(t);
        }
        Self {
            client,
            store,
            tx,
            screen: if has_token { Screen::Series } else { Screen::Phone },
            dialing_code: "+91".into(),
            phone_input: String::new(),
            otp_input: String::new(),
            otp_field_code: false,
            otp_hash: String::new(),
            busy: false,
            status: String::new(),
            error: None,
            notice: None,
            spinner_frame: 0,
            series: Vec::new(),
            series_page: 0,
            series_sel: 0,
            series_auto_loaded: false,
            series_exhausted: false,
            episodes: Vec::new(),
            episodes_page: 0,
            ep_sel: 0,
            episodes_exhausted: false,
            current_series: None,
            player: None,
            playing_title: String::new(),
        }
    }

    pub fn handle_msg(&mut self, msg: Msg) {
        self.busy = false;
        self.status.clear();
        match msg {
            Msg::SendOtpDone(Ok(hash)) => {
                self.otp_hash = hash.unwrap_or_default();
                self.screen = Screen::Otp;
                self.otp_input.clear();
                self.notice = Some("OTP bhej diya — SMS check karein".into());
            }
            Msg::SendOtpDone(Err(e)) => self.error = Some(format!("sendOtp fail: {}", e)),
            Msg::VerifyOtpDone(Ok(token)) => {
                self.client.set_token(token.clone());
                if let Err(e) = self.store.save_token(&token) {
                    self.error = Some(format!("Token save fail: {}", e));
                }
                self.series.clear();
                self.series_auto_loaded = false;
                self.series_exhausted = false;
                self.episodes_exhausted = false;
                self.screen = Screen::Series;
                self.notice = Some("Login ho gaya".into());
            }
            Msg::VerifyOtpDone(Err(e)) => self.error = Some(format!("verifyOtp fail: {}", e)),
            Msg::SeriesLoaded(Ok(list)) => {
                self.series_exhausted = list.len() < SERIES_PAGE_SIZE;
                self.series = list;
                self.series_sel = self.series_sel.min(self.series.len().saturating_sub(1));
            }
            Msg::SeriesLoaded(Err(e)) => {
                if is_auth_expired(&e) {
                    self.session_expired();
                } else {
                    self.error = Some(format!("series load fail: {}", e));
                }
            }
            Msg::EpisodesLoaded { series_id, result } => {
                let matches_current = self
                    .current_series
                    .as_ref()
                    .map(|s| s.id == series_id)
                    .unwrap_or(false);
                if !matches_current {
                    return;
                }
                match result {
                    Ok(list) => {
                        self.episodes_exhausted = list.len() < EPISODES_PAGE_SIZE;
                        self.episodes = list;
                        self.ep_sel = 0;
                        self.screen = Screen::Episodes;
                    }
                    Err(e) => {
                        if is_auth_expired(&e) {
                            self.session_expired();
                        } else {
                            self.error = Some(format!("episodes load fail: {}", e));
                        }
                    }
                }
            }
            Msg::EpisodeLoaded { series_id, result } => {
                let matches_current = self
                    .current_series
                    .as_ref()
                    .map(|s| s.id == series_id)
                    .unwrap_or(false);
                if !matches_current {
                    return;
                }
                match result {
                    Ok(detail) => match self.launch_player(detail) {
                        Ok(()) => {}
                        Err(e) => self.error = Some(format!("player start fail: {}", e)),
                    },
                    Err(e) => {
                        if is_auth_expired(&e) {
                            self.session_expired();
                        } else {
                            self.error = Some(e);
                        }
                    }
                }
            }
            Msg::MpvExited => {
                self.player = None;
                self.playing_title.clear();
                self.screen = Screen::Episodes;
                self.notice = Some("Playback band ho gaya".into());
            }
        }
    }

    fn launch_player(&mut self, detail: EpisodeDetail) -> anyhow::Result<()> {
        if detail.episode.is_unlocked == Some(false) {
            self.error = Some(
                "Ye episode locked hai — aapke account ke liye server ne access nahi diya".into(),
            );
            return Ok(());
        }
        let uri = detail
            .video_url
            .clone()
            .or_else(|| detail.cookie_plain.clone().filter(|c| c.starts_with("http")));
        let uri = match uri {
            Some(u) => u,
            None => {
                self.error = Some("Stream URL nahi mila response mein".into());
                return Ok(());
            }
        };
        let cookie = detail.cookie_plain.clone().filter(|c| !c.is_empty());
        let mpv = crate::player::find_mpv().ok_or_else(|| {
            anyhow::anyhow!("mpv nahi mila PATH mein — install karein: sudo apt install mpv")
        })?;
        let title = format!(
            "{} — EP{}",
            self.current_series
                .as_ref()
                .map(|s| s.title.clone())
                .unwrap_or_default(),
            detail
                .episode
                .number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".into())
        );
        let handle = PlayerHandle::spawn(&mpv, &uri, cookie.as_deref(), &title)?;
        self.playing_title = title;
        self.player = Some(handle);
        self.screen = Screen::Playing;
        Ok(())
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if self.screen == Screen::Series
            && self.series.is_empty()
            && !self.busy
            && !self.series_auto_loaded
        {
            self.load_series(0);
        }
        if self.screen == Screen::Playing {
            let running = self.player.as_mut().map(|p| p.is_running()).unwrap_or(false);
            if !running {
                let _ = self.tx.send(Msg::MpvExited);
            }
        }
    }

    pub fn load_series(&mut self, page: u32) {
        if self.busy {
            return;
        }
        self.series_page = page;
        self.series_auto_loaded = true;
        self.busy = true;
        self.status = "Series load ho rahi hain…".into();
        let client = self.client.clone();
        let tx = self.tx.clone();
        let page = self.series_page;
        std::thread::spawn(move || {
            let r = client.get_series(SERIES_PAGE_SIZE as u32, page).map_err(|e| e.to_string());
            let _ = tx.send(Msg::SeriesLoaded(r));
        });
    }

    pub fn load_episodes(&mut self, page: u32) {
        if self.busy {
            return;
        }
        let Some(series) = self.current_series.clone() else {
            return;
        };
        self.episodes_page = page;
        self.busy = true;
        self.status = "Episodes load ho rahe hain…".into();
        let client = self.client.clone();
        let tx = self.tx.clone();
        let (sid, page) = (series.id.clone(), self.episodes_page);
        std::thread::spawn(move || {
            let r = client
                .get_series_episodes(&sid, page, EPISODES_PAGE_SIZE as u32)
                .map_err(|e| e.to_string());
            let _ = tx.send(Msg::EpisodesLoaded { series_id: sid, result: r });
        });
    }

    pub fn open_episode(&mut self) {
        if self.busy {
            return;
        }
        let Some(series) = self.current_series.clone() else {
            return;
        };
        let Some(ep) = self.episodes.get(self.ep_sel).cloned() else {
            return;
        };
        let num = ep.number.unwrap_or((self.ep_sel + 1) as f64);
        self.busy = true;
        self.status = "Episode data + cookie decrypt ho raha hai…".into();
        let client = self.client.clone();
        let tx = self.tx.clone();
        let sid = series.id.clone();
        std::thread::spawn(move || {
            let r = client
                .get_series_episode(&sid, num, "")
                .map_err(|e| e.to_string());
            let _ = tx.send(Msg::EpisodeLoaded { series_id: sid, result: r });
        });
    }

    pub fn logout(&mut self) {
        let _ = self.store.clear_token();
        self.client.clear_token();
        self.series.clear();
        self.episodes.clear();
        self.series_auto_loaded = false;
        self.series_exhausted = false;
        self.episodes_exhausted = false;
        self.error = None;
        self.screen = Screen::Phone;
        self.notice = Some("Logout ho gaya".into());
    }

    fn session_expired(&mut self) {
        let _ = self.store.clear_token();
        self.client.clear_token();
        self.series.clear();
        self.episodes.clear();
        self.player = None;
        self.playing_title.clear();
        self.screen = Screen::Phone;
        self.error = Some("Session expire ho gayi — dobara login karein".into());
    }
}

pub const SERIES_PAGE_SIZE: usize = 20;
pub const EPISODES_PAGE_SIZE: usize = 50;

fn is_auth_expired(e: &str) -> bool {
    e.contains("Session expire")
}

pub fn phone_with_code(code: &str, number: &str) -> String {
    let code = code.trim();
    let code = if code.starts_with('+') {
        code.to_string()
    } else {
        format!("+{}", code)
    };
    format!("{}{}", code, number.trim())
}

pub fn is_valid_phone(number: &str) -> bool {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.len() >= 10
}
