# Vitrolinha

[Português](README.md) · **English**

Your screen's record player: a music flyout for Windows that looks like a vintage turntable. It shows
what's playing (any app that appears in the Windows media controls) in a **popup** when the track
changes and in a **panel** for your second monitor.

Tauri 2 (Rust) + plain TypeScript/CSS + Vite. The interface is in Portuguese and English (it follows
the Windows language, or pick one in the tray menu).

## Features

- Popup on track change, with previous · play/pause · next
- Panel on a full monitor or as a free window (mini turntable frame)
- **Progress line**: follows the track and, if the player allows it, jumps wherever you click. You can
  also **grab the tonearm and drop the needle on another spot of the record**
- **Accent color**: taken from the cover art (default), classic amber, seven presets or any color on a **color wheel** (plus adjustable background glow and film grain); high-resolution cover (iTunes Search, optional)
- Synced lyrics (lrclib.net, optional) with the current line lit up
- Record effects (shimmer/dust that rotate, tonearm wobble) at 3 levels; **16, 33 or 45 rpm** speed
- Optional **old record sound** over the music: surface hiss, ticks, scratches and the "tec" of a groove once per turn of the record, in 3 levels (generated on the fly, only while the panel is open)
- Paused for a while, the **record goes back into its sleeve**
- **Share card**: an image with the turntable and the current track (or your most played records from
  the shelf), saved to Pictures\Vitrolinha and already copied so you can paste it in a chat
- Shelf with the latest records you listened to (history stays on this computer)
- **Sleep timer**: pauses the music after 15 to 90 minutes
- Who is in your Discord voice call, on the full screen (optional, needs setup)
- **"Listening to" on your Discord profile** (optional) and an **OBS overlay** (optional)
- System volume (small, in the top bar), media source filter (e.g. YouTube Music only), global shortcuts
- Options grouped by function, in the panel's options screen (three-lines button) and in the tray menu

## Install

Download the `Vitrolinha_*_x64-setup.exe` installer from
[Releases](https://github.com/guNaoki/vitrolinha/releases) and run it. It installs for your user only
(no administrator needed), adds a Start menu shortcut and can be uninstalled from Windows. There is
also a portable `.zip`. To start with Windows, use "Start with Windows" in the tray menu.

Windows 10 or 11, 64-bit. Built and tested on Windows 11.

> **Did Windows say "Windows protected your PC"?** The installer is not code-signed yet, and
> SmartScreen warns about new apps. Click **More info → Run anyway**. To check the file, compare its
> SHA256 with `SHA256SUMS.txt` on the same page; the installer is built by GitHub Actions from this
> source code.

## Something went wrong?

Right-click the tray icon → **Open the logs folder** and paste the end of `vitrolinha.log` into an
[issue](https://github.com/guNaoki/vitrolinha/issues/new/choose). The log has only errors and app
states, each line with date and time; it does not include what you listen to. It lives in
`%LOCALAPPDATA%\app.vitrolinha\logs` (up to 1 MB, plus the previous file).

## Build from source

You need Node, Rust and the Visual Studio build tools (MSVC).

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # installer in src-tauri\target\release\bundle\nsis
```

That script also strips your machine's paths (Windows user name, Cargo folder) from the exe. For daily
use without the installer, `npx tauri build --no-bundle` builds just the exe (always with
`--no-bundle`: a plain `cargo build --release` makes an exe that points to `localhost:1420`) and
`scripts\install.ps1` copies it to `%LOCALAPPDATA%\Programs\Vitrolinha` and creates the shortcuts.

## OBS overlay

Turn on **OBS overlay** in the tray menu, then in OBS add a **Browser** source with the address
`http://localhost:7421` (suggested size 520 × 170; the background is transparent). It shows the
current record spinning, with title, artist and progress, and fades out when nothing is playing.
The server listens on this computer only (127.0.0.1) and answers only to `localhost`.

## "Listening to" on Discord

With **Show on Discord what I'm listening to** on, your Discord profile shows the track title and
artist, plus the progress. It needs the same Client ID as the call list (see below). It is off by
default, and it only works with the Discord desktop app open.

## Discord call on the full screen

Shows, in the top-left corner of the panel (only in "fill the monitor" mode), who is in the voice
channel you are in: avatar, name, a glow on whoever is talking and a muted/deafened icon. Read-only:
it does not control the call. It does not show in the free window.

Discord only hands this to an authorized app, so you need to create **your own** app (free, once):

1. At <https://discord.com/developers/applications> → **New Application** (any name).
2. **OAuth2** → under **Redirects**, add `http://localhost` and save. Copy the **Client ID** and, under
   **Reset Secret**, the **Client Secret**.
3. Run `powershell -ExecutionPolicy Bypass -File scripts\discord-setup.ps1`: it asks for the Client ID
   and the Secret, saves them to `%APPDATA%\app.vitrolinha\discord.json` (a Discord-only file the app
   does not rewrite) and turns the option on. If you prefer editing by hand, the file has the keys
   `enabled`, `clientId`, `clientSecret` and, if you need another redirect, `redirectUri`.
4. With Discord open, it asks for an authorization once: accept. The token renews itself.

**Without a secret (public client, PKCE).** If you turn on **OAuth2 → Public Client** in the Discord
app, the Client Secret is not needed: leave it empty in the script (or drop the `clientSecret` key) and
Vitrolinha authorizes with PKCE.

## Development

```powershell
npm install
npm run dev            # front end only, in the browser (no Rust; Tauri calls are mocked)
cd src-tauri; cargo test
```

After changing dependencies (`Cargo.lock` or `package-lock.json`), regenerate the license notices:
`python -I scripts/third-party.py` (CI checks that `THIRD-PARTY-LICENSES.txt` is up to date).

## License

[MIT](LICENSE). The licenses of the dependencies and fonts are in
[THIRD-PARTY-LICENSES.txt](THIRD-PARTY-LICENSES.txt), which also ships inside the installer.
Not affiliated with Discord, Apple (iTunes) or lrclib.

## Privacy

Nothing leaves your computer by default, except, if turned on in the tray menu: lyrics lookup (artist,
title, album and length → lrclib.net) and large cover lookup (artist and title → itunes.apple.com).
The shelf lives in `%APPDATA%\app.vitrolinha\shelf` and can be turned off and erased from the menu.
The diagnostic log stays on this computer. Share cards are made and saved locally, and only go where
you paste them.

The Discord call (off by default) talks only to the Discord app on your own computer. On the network:
exchanging the authorization code for a token (discord.com) and profile pictures, which the panel loads
from Discord's CDN. The client secret and the token are stored as plain text in `discord.json`, in the
app's data folder. "Listening to" (off by default) sends the title and artist to your own Discord app,
which shows them on your profile.
