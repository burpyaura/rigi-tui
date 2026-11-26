# rigi-tui

Rigi TV (com.litvibes / Rigi TV Android app) ke streaming ke liye terminal client.
Reverse-engineered protocol par based — sirf streaming: OTP login → series browse → episodes → mpv playback.

## Build

```sh
cargo build --release
```

Requirement: Rust (2021 edition) aur `mpv` PATH mein hona chahiye playback ke liye.

## Run

```sh
./target/release/rigi-tui
```

### Screens / keys

| Screen | Keys |
|---|---|
| Login | digits daalo, `Tab` = dialing code ↔ number, `Enter` = OTP bhejo |
| OTP | 6 digit daalo, `Enter` = verify, `r` = resend, `Esc` = back |
| Series | `↑↓/j/k` move, `Enter` = episodes, `n`/`b` = next/prev page, `r` = reload, `l` = logout, `q` = quit |
| Episodes | `↑↓/j/k` move, `Enter` = play, `r` reload, `n` next page, `Esc` = back |
| Playing | mpv external window mein chalta hai; `q` TUI mein wapas |

## Streaming protocol (APK se reverse-engineered)

- Base: `https://backboard.rigi.club/api/v1`, headers: `x-platform: ANDROID` + app/device headers
- `POST /auth/sendOtp` — body `{phone, dialingCode, resend, timestamp, hmac}`;
  `hmac = HMAC-SHA256(canonical JSON, embedded key)` (key order matters)
- `POST /auth/verifyOtp` → `{jwtToken}` → `Authorization: Bearer <token>`
- `GET /series/all`, `GET /series/episodes`, `GET /series/episode`
- Episode response ka `cookie` field AES-256-CBC (`IV:CIPHERTEXT` hex) encrypted hai —
  decrypt karke mpv ko `Cookie:` HTTP header ke through pass hota hai

## Security

- Token `~/.config/rigi-tui/credentials.json` mein **0600** perms ke saath
- Cookie kabhi process argv ya logs mein nahi jaata — mpv IPC socket (0700 temp dir) se pass hota hai
- Logs mein sirf redacted/truncated response text; `RIGI_TUI_DEBUG=1` sirf JSON *shape*
  (keys + types, values nahi) `~/.config/rigi-tui/debug.log` mein likhta hai

## Limits / notes

- **Entitlement respect karta hai:** locked episodes UI mein locked dikhte hain aur
  server ne jo access nahi diya wo client khud bypass nahi karta.
- `nonceForPlayIntegrity` TUI mein khali jaata hai (Play Integrity attestation sirf
  Android app ke liye hai) — server agar nonce enforce kare to play reject ho sakta hai.
- API response shapes runtime par verify karna hoga; parser tolerant hai aur mismatch par
  `RIGI_TUI_DEBUG=1` se debug.log mein shape mil jaata hai.
