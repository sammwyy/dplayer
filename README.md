# Dplayer

A native video and music player for Linux, built on
[CreamUI](https://github.com/sammwyy/creamui).

## Features

- Video and audio playback: mp4, mkv, webm, avi, mov, mp3, flac, ogg,
  opus, m4a, wav and anything else ffmpeg understands.
- Floating transport card, centered with a configurable maximum width, that
  slides in and out over video: seek bar (click or drag), play/pause,
  previous / next, shuffle, repeat (off → all → one), playback speed
  (0.5×–2×), volume slider and mute.
- Queue panel with add / remove / click-to-play.
- Music view with the embedded cover art, title and "Album - Artist" over a
  blurred backdrop of the artwork.
- Titlebar built on CreamUI's `AppTitlebar`: settings button, the video title
  in the center, and window controls drawn by the compositor (hybrid) or by
  the app as a fallback. Everything mirrors when the controls sit on the left.
- Automatic accent color taken from the cover art or the video picture.
- Desktop media controls: MPRIS 2 on Linux (shell media widgets, media keys,
  `playerctl`) and System Media Transport Controls on Windows (media flyout,
  lock screen, hardware keys). Title, artist, album, cover art and timeline
  are published; play/pause, stop, next/previous, seeking, volume, rate,
  shuffle and repeat can be driven from outside.
- Translucent blurred window and live system-theme reload.
- Keyboard-first: `space` play/pause · `←` `→` seek 5 s (`shift` = 30 s) ·
  `↑` `↓` volume · `f` maximize · `m` mute · `[` `]` speed ·
  `n`/`p` next/previous · `l` repeat · `s` shuffle · `t` queue ·
  `o` open · `,` settings · `q`/`Esc` quit.
- Settings window (`dplayer --settings`, or the gear in the top-left corner):
  available playback backends, startup volume / speed / repeat / shuffle,
  autoplay, seek steps, controls auto-hide delay, animations, bar width and
  style, accent color (automatic, system or a preset), music background and
  cover shape. Stored in `~/.config/dplayer/config.toml` and applied live to
  open players.

## Usage

```sh
dplayer [FILE|DIRECTORY ...]
dplayer --settings
```

Directories are expanded to their playable files in sorted order; with no
arguments the player opens on a welcome screen.

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

## Roadmap

- Drag & drop onto the window
- Subtitle and audio track selection
- Embedded picture display while seeking
