use super::{extract_array, get_bool, get_num, get_str, RigiClient};
use anyhow::{anyhow, Result};
use serde_json::Value;

#[derive(Clone)]
pub struct Series {
    pub id: String,
    pub title: String,
    pub total_episodes: Option<f64>,
}

#[derive(Clone)]
pub struct Episode {
    pub number: Option<f64>,
    pub title: String,
    pub is_unlocked: Option<bool>,
}

pub struct EpisodeDetail {
    pub episode: Episode,
    pub cookie_plain: Option<String>,
    pub video_url: Option<String>,
}

const SERIES_ID_KEYS: &[&str] = &["id", "_id", "seriesId", "series_id"];
const TITLE_KEYS: &[&str] = &["title", "name", "seriesTitle", "seriesName"];
const EP_ID_KEYS: &[&str] = &["id", "_id", "episodeId", "episode_id"];
const EP_NUM_KEYS: &[&str] = &["episodeNumber", "number", "episodeNo", "episode"];
const EP_UNLOCK_KEYS: &[&str] = &["isUnlocked", "unlocked", "isFree", "isUnlockedForUser"];
const VIDEO_URL_KEYS: &[&str] = &[
    "videoUrl",
    "url",
    "streamUrl",
    "video",
    "sourceUrl",
    "mediaUrl",
    "playbackUrl",
    "hlsUrl",
    "videoLink",
    "link",
];
const COOKIE_KEYS: &[&str] = &["cookie"];

impl Series {
    pub fn from_value(v: &Value) -> Option<Self> {
        let id = get_str(v, SERIES_ID_KEYS)?;
        let title = get_str(v, TITLE_KEYS).unwrap_or_else(|| "(untitled)".into());
        Some(Self {
            id,
            title,
            total_episodes: get_num(v, &["totalEpisodes", "episodesCount", "episodeCount"]),
        })
    }
}

impl Episode {
    pub fn from_value(v: &Value) -> Option<Self> {
        get_str(v, EP_ID_KEYS)?;
        let title = get_str(v, TITLE_KEYS).unwrap_or_else(|| "(untitled)".into());
        Some(Self {
            number: get_num(v, EP_NUM_KEYS),
            title,
            is_unlocked: get_bool(v, EP_UNLOCK_KEYS),
        })
    }
}

impl RigiClient {
    pub fn get_series(&self, limit: u32, page: u32) -> Result<Vec<Series>> {
        let resp = self.get(&format!("/series/all?limit={}&page={}", limit, page), true)?;
        let arr = extract_array(&resp, &["series", "items", "results", "list"]);
        let list: Vec<Series> = arr.iter().filter_map(Series::from_value).collect();
        if list.is_empty() {
            return Err(anyhow!(
                "Series list khali/nahi samjhi — RIGI_TUI_DEBUG=1 se shape dekhein"
            ));
        }
        Ok(list)
    }

    pub fn get_series_episodes(&self, series_id: &str, page: u32, limit: u32) -> Result<Vec<Episode>> {
        let resp = self.get(
            &format!(
                "/series/episodes?seriesId={}&page={}&limit={}",
                series_id, page, limit
            ),
            true,
        )?;
        let arr = extract_array(&resp, &["episodes", "items", "results", "list", "data"]);
        let list: Vec<Episode> = arr.iter().filter_map(Episode::from_value).collect();
        if list.is_empty() {
            return Err(anyhow!(
                "Episodes khali/nahi samjhe — RIGI_TUI_DEBUG=1 se shape dekhein"
            ));
        }
        Ok(list)
    }

    pub fn get_series_episode(&self, series_id: &str, episode_number: f64, nonce: &str) -> Result<EpisodeDetail> {
        let resp = self.get(
            &format!(
                "/series/episode?seriesId={}&episodeNumber={}&nonceForPlayIntegrity={}",
                series_id, episode_number, nonce
            ),
            true,
        )?;
        let ep_val = resp.get("episode").cloned().unwrap_or_else(|| resp.clone());
        let episode = Episode::from_value(&ep_val)
            .ok_or_else(|| anyhow!("Episode object nahi mila response mein"))?;
        let cookie_enc = get_str(&ep_val, COOKIE_KEYS);
        let mut cookie_plain = None;
        if let Some(c) = &cookie_enc {
            match crate::api::crypto::decrypt_aes(c, crate::api::crypto::EPISODE_COOKIE_KEY) {
                Ok(dec) => cookie_plain = Some(dec),
                Err(_) => cookie_plain = Some(c.clone()),
            }
        }
        let video_url = get_str(&ep_val, VIDEO_URL_KEYS);
        if cookie_plain.is_none() && video_url.is_none() {
            return Err(anyhow!(
                "Episode data mein na cookie hai na video URL — entitlement check karein"
            ));
        }
        Ok(EpisodeDetail {
            episode,
            cookie_plain,
            video_url,
        })
    }
}
