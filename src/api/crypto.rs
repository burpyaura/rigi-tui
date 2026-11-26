use anyhow::{anyhow, Result};
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
type HmacSha256 = Hmac<Sha256>;

pub const OTP_HMAC_KEY: &str = "Xa7vKp9LmQz38WdUrNfC2YbTgJEoV5Hs";
pub const EPISODE_COOKIE_KEY: &str = "hAMEuLWLexKQ4PIbpIhFQpCeh6AW31U0";
const BINARY_KEY_SEED: &str = "/y8tfYLMX0irQW4qKgu8FHD3nSYxVm5A+oeAPxVMXYU=";

pub fn hmac_sha256_hex(data: &[u8], key: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(data);
    hex::encode(mac.finalize().into_bytes())
}

pub fn send_otp_payload_signature(payload_json: &str) -> String {
    hmac_sha256_hex(payload_json.as_bytes(), OTP_HMAC_KEY)
}

pub fn decrypt_aes(encrypted_text: &str, key: &str) -> Result<String> {
    let parts: Vec<&str> = encrypted_text.split(':').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid encrypted text format. Expected format: IV:EncryptedText"
        ));
    }
    if key.len() != 32 {
        return Err(anyhow!(
            "Encryption key must be 32 characters (AES-256 requires 32-byte key)."
        ));
    }
    let iv = hex::decode(parts[0].trim())?;
    let ct = hex::decode(parts[1].trim())?;
    if iv.len() != 16 {
        return Err(anyhow!("IV must be 16 bytes, got {}", iv.len()));
    }
    let buf = ct;
    let pt = Aes256CbcDec::new_from_slices(key.as_bytes(), &iv)
        .map_err(|e| anyhow!("AES key/iv fail: {}", e))?
        .decrypt_padded_vec_mut::<Pkcs7>(&buf)
        .map_err(|_| anyhow!("AES-CBC decrypt ya PKCS7 pad fail"))?;
    String::from_utf8(pt).map_err(|_| anyhow!("Decrypted text valid UTF-8 nahi hai"))
}

fn derive_binary_key() -> Vec<u8> {
    let digest = Sha256::digest(BINARY_KEY_SEED.as_bytes());
    let hex_str = hex::encode(digest);
    hex_str.as_bytes()[..32].to_vec()
}

pub fn decrypt_binary_response(data: &[u8]) -> Result<serde_json::Value> {
    if data.len() < 16 {
        return Err(anyhow!(
            "Input data too short - must be at least 16 bytes for IV"
        ));
    }
    let iv = &data[..16];
    let ct = &data[16..];
    if ct.is_empty() {
        return Err(anyhow!("No encrypted data found after IV"));
    }
    let key = derive_binary_key();
    let buf = ct.to_vec();
    let pt = Aes256CbcDec::new_from_slices(&key, iv)
        .map_err(|e| anyhow!("AES key/iv fail: {}", e))?
        .decrypt_padded_vec_mut::<Pkcs7>(&buf)
        .map_err(|_| anyhow!("AES-CBC decrypt ya PKCS7 pad fail"))?;
    let text = String::from_utf8_lossy(&pt);
    let cleaned: String = text
        .chars()
        .filter(|c| *c != '\0' && !c.is_control())
        .collect();
    let trimmed = cleaned.trim();
    serde_json::from_str(trimmed).map_err(|e| anyhow!("Decrypted JSON parse fail: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_tc2() {
        let sig = hmac_sha256_hex(b"what do ya want for nothing?", "Jefe");
        assert_eq!(
            sig,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn decrypt_roundtrip() {
        use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
        type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
        let key = EPISODE_COOKIE_KEY;
        let iv: [u8; 16] = rand::random();
        let msg = b"https://cdn.example.com/stream/index.m3u8?tok=abc";
        let buf = msg.to_vec();
        let ct = Aes256CbcEnc::new_from_slices(key.as_bytes(), &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(&buf);
        let enc = format!("{}:{}", hex::encode(iv), hex::encode(&ct));
        let dec = decrypt_aes(&enc, key).unwrap();
        assert_eq!(dec, String::from_utf8_lossy(msg));
    }

    #[test]
    fn decrypt_rejects_bad_format() {
        assert!(decrypt_aes("no-colon-here", EPISODE_COOKIE_KEY).is_err());
        assert!(decrypt_aes("aa:bb", EPISODE_COOKIE_KEY).is_err());
        assert!(decrypt_aes("00112233445566778899001122334455:deadbeef", "short").is_err());
    }

    #[test]
    fn binary_key_is_32_bytes() {
        assert_eq!(derive_binary_key().len(), 32);
    }
}
