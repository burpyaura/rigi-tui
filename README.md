# rigi-tui

Rigi TV (com.litvibes / Rigi TV Android app) ke streaming ke liye terminal client.
Reverse-engineered protocol par based — sirf streaming: OTP login → series browse → episodes → mpv playback.

## Termux (Android)

Prebuilt binary (koi build nahi — ~15MB download):

```sh
pkg update -y && pkg install mpv -y
curl -L -o r.tgz https://github.com/burpyaura/rigi-tui/releases/latest/download/rigi-tui-aarch64-android.tar.gz
tar xzf r.tgz && chmod +x rigi-tui
./rigi-tui
```

- mpv zaroori hai (external player) — `pkg install mpv`
- Lambi playback ke dauran `termux-wake-lock` chala lo
- mpv video Android ke fullscreen surface pe khulega; band karne ke liye mpv ke andar `q`

### Source se build (fallback)

```sh
pkg update -y && pkg install git rust clang binutils mpv -y
git clone https://github.com/burpyaura/rigi-tui.git
cd rigi-tui
cargo build --release
./target/release/rigi-tui
```

- `clang` zaroori hai (TLS crate ka C code) aur `binutils` linker ke liye
- Pehla build 10-20 min le sakta hai; RAM kam aaye to: `CARGO_BUILD_JOBS=2 cargo build --release`
- Prebuilt binary `aarch64` (2018+ phones) ke liye hai; armv7 ya x86_64 ho to source build karo

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
