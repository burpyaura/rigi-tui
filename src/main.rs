mod app;
mod api;
mod player;
mod store;
mod ui;

use app::{phone_with_code, App, Msg};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use std::sync::mpsc;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let store = store::Store::new()?;
    let device_id = store.device_id();
    let saved_token = store.load().token.clone();
    let client = api::RigiClient::new(device_id, saved_token);
    let (tx, rx) = mpsc::channel::<Msg>();
    let mut app = App::new(client, store, tx);

    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app, &rx);
    if let Some(p) = app.player.as_mut() {
        p.stop();
    }
    ratatui::restore();
    result
}

fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mpsc::Receiver<Msg>,
) -> anyhow::Result<()> {
    loop {
        if event::poll(Duration::from_millis(80))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press && handle_key(app, k) {
                    return Ok(());
                }
            }
        }
        while let Ok(msg) = rx.try_recv() {
            app.handle_msg(msg);
        }
        app.tick();
        terminal.draw(|f| ui::draw(f, app))?;
    }
}

fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    use KeyCode::*;
    if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
        && matches!(key.code, Char('c') | Char('C'))
    {
        return true;
    }
    match app.screen {
        app::Screen::Phone => match key.code {
            Char(c) if c.is_ascii_digit() || (c == '+' && app.otp_field_code) => {
                if app.otp_field_code {
                    if app.dialing_code.len() < 5 {
                        app.dialing_code.push(c);
                    }
                } else if app.phone_input.len() < 15 {
                    app.phone_input.push(c);
                }
                app.error = None;
            }
            Backspace => {
                if app.otp_field_code {
                    app.dialing_code.pop();
                } else {
                    app.phone_input.pop();
                }
            }
            Tab => app.otp_field_code = !app.otp_field_code,
            Enter => {
                if app.busy {
                    return false;
                }
                let code = app.dialing_code.trim();
                let code_ok =
                    !code.is_empty() && code.trim_start_matches('+').chars().all(|d| d.is_ascii_digit()) && code.len() <= 5;
                if !code_ok {
                    app.error = Some("Dialing code galat hai (jaise +91)".into());
                    return false;
                }
                if !app::is_valid_phone(&app.phone_input) {
                    app.error = Some("Kam se kam 10 digits daalein".into());
                    return false;
                }
                app.busy = true;
                app.status = "OTP maang rahe hain…".into();
                let client = app.client.clone();
                let tx = app.tx.clone();
                let phone = phone_with_code(&app.dialing_code, &app.phone_input);
                let code = app.dialing_code.trim_start_matches('+').to_string();
                std::thread::spawn(move || {
                    let dial = format!("+{}", code);
                    let r = client
                        .send_otp(&phone, &dial, false)
                        .map(|res| res.hash)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Msg::SendOtpDone(r));
                });
            }
            _ => {}
        },
        app::Screen::Otp => match key.code {
            Char(c) if c.is_ascii_digit() && app.otp_input.len() < 6 => {
                app.otp_input.push(c);
                app.error = None;
            }
            Backspace => {
                app.otp_input.pop();
            }
            Char('r') | Char('R') => {
                if app.busy {
                    return false;
                }
                app.busy = true;
                app.status = "OTP dobara bhej rahe hain…".into();
                let client = app.client.clone();
                let tx = app.tx.clone();
                let phone = phone_with_code(&app.dialing_code, &app.phone_input);
                let code = app.dialing_code.trim_start_matches('+').to_string();
                std::thread::spawn(move || {
                    let dial = format!("+{}", code);
                    let r = client
                        .send_otp(&phone, &dial, true)
                        .map(|res| res.hash)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Msg::SendOtpDone(r));
                });
            }
            Esc => app.screen = app::Screen::Phone,
            Enter => {
                if app.busy {
                    return false;
                }
                if app.otp_input.len() != 6 {
                    app.error = Some("6 digit OTP daalein".into());
                    return false;
                }
                if app.otp_hash.is_empty() {
                    app.error = Some("Hash nahi mila — pehle OTP bhejein".into());
                    return false;
                }
                app.busy = true;
                app.status = "Verify kar rahe hain…".into();
                let client = app.client.clone();
                let tx = app.tx.clone();
                let phone = phone_with_code(&app.dialing_code, &app.phone_input);
                let code = app.dialing_code.trim_start_matches('+').to_string();
                let otp = app.otp_input.clone();
                let hash = app.otp_hash.clone();
                std::thread::spawn(move || {
                    let dial = format!("+{}", code);
                    let r = client
                        .verify_otp(&phone, &dial, &otp, &hash)
                        .map(|res| res.token)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(Msg::VerifyOtpDone(r));
                });
            }
            _ => {}
        },
        app::Screen::Series => match key.code {
            Up | Char('k') => {
                if app.series_sel > 0 {
                    app.series_sel -= 1;
                }
            }
            Down | Char('j') => {
                if app.series_sel + 1 < app.series.len() {
                    app.series_sel += 1;
                }
            }
            Char('n') => {
                if !app.series_exhausted {
                    let page = app.series_page + 1;
                    app.load_series(page);
                }
            }
            Char('b') => {
                let page = app.series_page.saturating_sub(1);
                app.load_series(page);
            }
            Char('r') | Char('R') => {
                let page = app.series_page;
                app.load_series(page);
            }
            Char('l') | Char('L') => app.logout(),
            Char('q') | Char('Q') => return true,
            Enter => {
                if let Some(s) = app.series.get(app.series_sel).cloned() {
                    app.current_series = Some(s);
                    app.episodes.clear();
                    app.episodes_page = 0;
                    app.ep_sel = 0;
                    app.load_episodes(0);
                }
            }
            _ => {}
        },
        app::Screen::Episodes => match key.code {
            Up | Char('k') => {
                if app.ep_sel > 0 {
                    app.ep_sel -= 1;
                }
            }
            Down | Char('j') => {
                if app.ep_sel + 1 < app.episodes.len() {
                    app.ep_sel += 1;
                }
            }
            Char('r') | Char('R') => {
                let page = app.episodes_page;
                app.load_episodes(page);
            }
            Char('n') => {
                if !app.episodes_exhausted {
                    let page = app.episodes_page + 1;
                    app.load_episodes(page);
                }
            }
            Esc | Char('h') => app.screen = app::Screen::Series,
            Enter => app.open_episode(),
            _ => {}
        },
        app::Screen::Playing => match key.code {
            Char('q') | Char('Q') | Esc => {
                if let Some(p) = app.player.as_mut() {
                    p.stop();
                }
                app.player = None;
                app.playing_title.clear();
                app.screen = app::Screen::Episodes;
            }
            _ => {}
        },
    }
    false
}
