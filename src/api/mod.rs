pub mod auth;
pub mod crypto;
pub mod series;

use anyhow::{anyhow, Result};
use serde_json::Value;

pub const API_BASE: &str = "https://backboard.rigi.club/api/v1";
pub const APP_VERSION: &str = "1.0.138";

#[derive(Clone)]
pub struct RigiClient {
    http: reqwest::blocking::Client,
    token: std::sync::Arc<std::sync::RwLock<Option<String>>>,
    device_id: String,
}

impl RigiClient {
    pub fn new(device_id: String, token: Option<String>) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent("okhttp/4.12.0")
            .build()
            .expect("http client");
        Self {
            http,
            token: std::sync::Arc::new(std::sync::RwLock::new(token)),
            device_id,
        }
    }

    pub fn set_token(&self, token: String) {
        *self.token.write().unwrap() = Some(token);
    }

    pub fn clear_token(&self) {
        *self.token.write().unwrap() = None;
    }

    pub fn base_headers(&self, auth: bool) -> Result<reqwest::header::HeaderMap> {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("Content-Type", "application/json".parse()?);
        h.insert("x-platform", "ANDROID".parse()?);
        h.insert("x-app-version-number", APP_VERSION.parse()?);
        h.insert("x-app-device-id", self.device_id.parse()?);
        h.insert("x-app-device-name", "rigi-tui".parse()?);
        h.insert("x-app-device-model", "terminal".parse()?);
        h.insert("x-app-device-manufacturer", "generic".parse()?);
        h.insert("x-app-device-system-version", "14".parse()?);
        h.insert("x-app-os-version", "14".parse()?);
        h.insert("x-app-device-memory", "4.0".parse()?);
        if auth {
            if let Some(t) = self.token.read().unwrap().as_ref() {
                h.insert("Authorization", format!("Bearer {}", t).parse()?);
            }
        }
        Ok(h)
    }

    pub fn get(&self, path: &str, auth: bool) -> Result<Value> {
        let url = format!("{}{}", API_BASE, path);
        let resp = self
            .http
            .get(&url)
            .headers(self.base_headers(auth)?)
            .send()?;
        self.finish(resp)
    }

    pub fn post(&self, path: &str, body: &Value, auth: bool) -> Result<Value> {
        let url = format!("{}{}", API_BASE, path);
        let resp = self
            .http
            .post(&url)
            .headers(self.base_headers(auth)?)
            .json(body)
            .send()?;
        self.finish(resp)
    }

    fn finish(&self, resp: reqwest::blocking::Response) -> Result<Value> {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            if status.as_u16() == 401 {
                self.clear_token();
                return Err(anyhow!("Session expire ho gayi (401) — dobara login karein"));
            }
            return Err(anyhow!("HTTP {} — {}", status.as_u16(), redact(&text)));
        }
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        json_shape("raw", &v);
        if v.is_null() {
            if text.len() >= 16 {
                let bytes = text.into_bytes();
                if let Ok(json) = crypto::decrypt_binary_response(&bytes) {
                    return Ok(json);
                }
            }
            return Err(anyhow!("Response samajh nahi aaya (empty/invalid JSON)"));
        }
        let u = unwrap_envelope(v);
        json_shape("unwrapped", &u);
        Ok(u)
    }
}

static DEBUG_PATH: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();

fn debug_path() -> Option<std::path::PathBuf> {
    DEBUG_PATH
        .get_or_init(|| {
            if std::env::var("RIGI_TUI_DEBUG").is_err() {
                return None;
            }
            dirs::config_dir().map(|d| d.join("rigi-tui").join("debug.log"))
        })
        .clone()
}

pub fn json_shape(tag: &str, v: &Value) {
    let Some(path) = debug_path() else {
        return;
    };
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {}", tag, shape_of(v));
    }
}

fn shape_of(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(_) => "number".into(),
        Value::String(s) => format!("string(len={})", s.len()),
        Value::Array(a) => format!(
            "[len={}; first={}]", 
            a.len(),
            a.first().map(shape_of).unwrap_or_else(|| "-".into())
        ),
        Value::Object(m) => {
            let keys: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("{}:{}", k, shape_of(val)))
                .collect();
            format!("{{{}}}", keys.join(", "))
        }
    }
}

pub fn unwrap_envelope(v: Value) -> Value {
    if let Value::Object(m) = &v {
        if let Some(d) = m.get("data") {
            if !d.is_null() {
                return d.clone();
            }
        }
    }
    v
}

pub fn redact(s: &str) -> String {
    if s.len() <= 300 {
        return s.to_string();
    }
    let mut end = 300;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

pub fn extract_array(v: &Value, keys: &[&str]) -> Vec<Value> {
    if let Value::Array(a) = v {
        return a.clone();
    }
    if let Value::Object(m) = v {
        for k in keys {
            if let Some(Value::Array(a)) = m.get(*k) {
                return a.clone();
            }
        }
        for val in m.values() {
            if let Value::Array(a) = val {
                if !a.is_empty() && a[0].is_object() {
                    return a.clone();
                }
            }
        }
    }
    Vec::new()
}

pub fn get_str(v: &Value, keys: &[&str]) -> Option<String> {
    if let Value::Object(m) = v {
        for k in keys {
            match m.get(*k) {
                Some(Value::String(s)) => return Some(s.clone()),
                Some(Value::Number(n)) => return Some(n.to_string()),
                _ => {}
            }
        }
    }
    None
}

pub fn get_bool(v: &Value, keys: &[&str]) -> Option<bool> {
    if let Value::Object(m) = v {
        for k in keys {
            match m.get(*k) {
                Some(Value::Bool(b)) => return Some(*b),
                Some(Value::Number(n)) => return Some(n.as_i64()? != 0),
                Some(Value::String(s)) => {
                    return Some(matches!(s.to_lowercase().as_str(), "true" | "1" | "yes"))
                }
                _ => {}
            }
        }
    }
    None
}

pub fn get_num(v: &Value, keys: &[&str]) -> Option<f64> {
    if let Value::Object(m) = v {
        for k in keys {
            match m.get(*k) {
                Some(Value::Number(n)) => return n.as_f64(),
                Some(Value::String(s)) => {
                    return s.trim().parse::<f64>().ok();
                }
                _ => {}
            }
        }
    }
    None
}
