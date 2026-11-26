use super::RigiClient;
use anyhow::{anyhow, Result};
use serde_json::json;
use crate::api::crypto;

pub struct SendOtpResult {
    pub hash: Option<String>,
}

pub struct VerifyOtpResult {
    pub token: String,
}

impl RigiClient {
    fn otp_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn send_otp(&self, phone_with_code: &str, dialing_code: &str, resend: bool) -> Result<SendOtpResult> {
        let ts = Self::otp_timestamp();
        let payload_json = format!(
            "{{\"phone\":\"{}\",\"dialingCode\":\"{}\",\"resend\":{},\"timestamp\":{}}}",
            phone_with_code, dialing_code, resend, ts
        );
        let hmac = crypto::send_otp_payload_signature(&payload_json);
        let body = json!({
            "phone": phone_with_code,
            "dialingCode": dialing_code,
            "resend": resend,
            "timestamp": ts,
            "hmac": hmac,
        });
        let resp = self.post("/auth/sendOtp", &body, false)?;
        let hash = super::get_str(&resp, &["hash", "otpHash", "verificationHash"]);
        Ok(SendOtpResult { hash })
    }

    pub fn verify_otp(&self, phone_with_code: &str, dialing_code: &str, otp: &str, hash: &str) -> Result<VerifyOtpResult> {
        let body = json!({
            "phone": phone_with_code,
            "dialingCode": dialing_code,
            "otp": otp,
            "hash": hash,
        });
        let resp = self.post("/auth/verifyOtp", &body, false)?;
        let token = super::get_str(&resp, &["jwtToken", "token", "authToken", "jwt"])
            .ok_or_else(|| anyhow!("VerifyOtp response mein token nahi mila"))?;
        Ok(VerifyOtpResult { token })
    }
}
