# Dplayer

A native video and music player for Linux, built on
[CreamUI](https://github.com/sammwyy/creamui).

## Features

- Video and audio playback: mp4, mkv, webm, avi, mov, mp3, flac, ogg,
  opus, m4a, wav and anything else ffmpeg understands.
- Floating transport card, centered with a configurable maximum width, that
  slides in and out over video: seek bar (click or drag), play/pause,
  previous / next, shuffle, repeat (off → all → one), playback speed
  (0.5×–2×), volume slider and mute. Its background opacity is adjustable
  independently of the controls; optional blur softens video or artwork
  underneath.
- The empty player keeps the same titlebar and transport card, with 00:00
  timestamps and disabled playback controls; open-file actions stay available.
- Separate **Open** (replace the queue and play now) and **Add Queue** (insert
  after the current track) buttons to the left of the transport controls.
  Their labels hide when space is limited. The queue panel keeps its
  add / remove / click-to-play controls.
- Subtitle selector in video mode: off, embedded tracks (including multiple
  languages), and same-name `.ass`, `.ssa`, `.srt` or `.vtt` files beside the video.
  Choose top/bottom placement, adjust outlines, or drag the on-screen handle
  while the subtitle menu is open. libmpv applies placement live; FFmpeg
  applies it when the drag ends. Position and outline reset for each new media file.
- Audio track selector for files with multiple audio streams.
- Music view with the embedded cover art, title and "Album - Artist" over a
  blurred backdrop of the artwork.
- Titlebar built on CreamUI's `AppTitlebar`: settings and queue buttons (plus
  subtitles and audio tracks in video mode), the video title
  in the center, and window controls drawn by the compositor (hybrid) or by
  the app as a fallback. Everything mirrors when the controls sit on the left.
- Automatic accent color taken from the cover art or the video picture.
- Desktop media controls: MPRIS 2 on Linux (shell media widgets, media keys,
  `playerctl`) and System Media Transport Controls on Windows (media flyout,
  lock screen, hardware keys). Title, artist, album, cover art and timeline
  are published; play/pause, stop, next/previous, seeking, volume, rate,
  shuffle and repeat can be driven from outside.
- Optional Discord Rich Presence (off by default): the current track shows as
  your Discord status — "Listening to"/"Watching" the player, title plus
  "by \<artist\>", the embedded cover art, and a live progress bar while
  playing. By default only files with artist metadata publish a presence;
  untagged files can instead show their filename without extension.
  Reconnects when Discord starts or restarts, and clears when the track stops
  or the player exits.
- Opaque windows and controls with live system-theme reload.
- Keyboard-first: `space` play/pause · `←` `→` seek 5 s (`shift` = 30 s) ·
  `↑` `↓` volume · `f` maximize · `m` mute · `[` `]` speed ·
  `n`/`p` next/previous · `l` repeat · `s` shuffle · `t` queue ·
  `o` open · `,` settings · `q`/`Esc` close the window.
- Settings window (`dplayer --settings`, or the gear in the top-left corner):
  available playback backends, startup volume / speed / repeat / shuffle,
  autoplay, seek steps, controls auto-hide delay, animations, bar width, style,
  background opacity and blur, optional background playback with a Linux tray
  icon, optional Discord presence, accent color
  (automatic, system or a preset), music background and
  cover shape. Stored in `~/.config/dplayer/config.toml` and applied live to
  open players.

## Usage

```sh
dplayer [FILE|DIRECTORY ...]
dplayer --player [FILE|DIRECTORY ...]
dplayer --settings
dplayer --quit
```

Directories are expanded to their playable files in sorted order; with no
arguments the player opens on a welcome screen.
By default closing the player window quits the player. Enable **Keep playing
in background** in Settings and restart the player to keep playback running
after closing the window. On Linux the tray offers Show player, Play / Pause,
Next and Quit. `dplayer` also restores the window without restarting the
track; passing files opens them in the existing player. `dplayer --quit`
stops the background player. Settings remain a separate window/process.

## Architecture

`ffprobe` reads stream metadata. Playback selects a backend at runtime:

- **libmpv** (when `libmpv.so.2`/`.so.1` is installed on Linux or
  `mpv-2.dll`/`libmpv-2.dll` is next to the executable on Windows): one
  persistent player per track. Seek, rate, pause and volume update that
  player without reopening the file. libmpv handles audio output; its
  software render API sends RGB frames to CreamUI. This removes process
  restarts but does **not** yet provide GPU texture interop or hardware video
  output; software rendering can be slower for large videos.
- **FFmpeg subprocess** fallback when libmpv is unavailable. It decodes
  audio for Rodio and video into raw frames:

```
ffprobe ── JSON metadata (streams, duration, tags)
ffmpeg  ── video: -f rawvideo rgba  → fd 3 → ImageData → creamui Image
        └─ audio: -f f32le 48kHz    → fd 4 → rodio Player → default device
```

The Playback settings show only detected backends and save the choice.
Changing it while a track is open prepares the new backend in the background
and resumes from the current position. If the saved backend disappears, the
player tries the other one. `DPLAYER_BACKEND=ffmpeg` overrides the setting to
force the fallback; `DPLAYER_BACKEND=mpv` requires libmpv and reports an error
if it cannot initialize. Neither library is linked into the executable at build time.
The FFmpeg backend still requires `ffmpeg` and `ffprobe` executables.

On the FFmpeg fallback:

- `-readrate N` paces demuxing and playback speed adds `atempo=N`.
  Raw video frames do not carry timestamps through the pipe, so this
  does not provide frame-accurate A/V synchronization under decoder stalls.
- Pause is `SIGSTOP` on the decoder plus `Player::pause()`; resume is
  `SIGCONT` + `play()`.
- Seek and rate changes prepare a new decoder on a background worker; the
  previous session is replaced on the UI thread only after the new process
  starts successfully. Every session is tagged with a generation so late
  frames/exits from superseded processes are dropped. The initial decoder
  also starts off the UI thread.
- Video is decoded to bounded RGBA frames (maximum 1600 px per side) with
  a two-frame channel; audio uses a small bounded PCM queue. This is a
  software decode / CPU-to-UI copy path, not hardware-accelerated video output.

## Install

```sh
cargo build --release
install -Dm755 target/release/dplayer /usr/bin/dplayer
install -Dm644 data/dplayer.desktop /usr/share/applications/dplayer.desktop
```

Runtime requirements: `ffprobe` and either a working libmpv installation or
`ffmpeg` plus a working audio output (ALSA/PipeWire/PulseAudio via `rodio`).
Linux builds link `libdbus-1` for
MPRIS (`dbus-devel` / `libdbus-1-dev` at build time).

Discord Rich Presence reports against the Discord application whose id is
hardcoded as `CLIENT_ID` in `src/discord.rs` (`DPLAYER_DISCORD_CLIENT_ID`
overrides it). IPC cannot receive image bytes, so cover art is sent as an
external image URL: each embedded cover is re-encoded as a small JPEG and
uploaded to [catbox.moe](https://catbox.moe) once per file (disable it under
Settings → Interface → Cover art). When a track has no artwork, or uploading
fails, the large image falls back to an art asset named `dplayer` uploaded to
the Discord application (Rich Presence → Art Assets in the
[developer portal](https://discord.com/developers/applications));
`DPLAYER_DISCORD_ASSET` points it at a different key.

## Roadmap

- Drag & drop onto the window
- Subtitle and audio track selection
- Embedded picture display while seeking
