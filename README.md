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
  startup volume / speed / repeat / shuffle, autoplay, seek steps, controls
  auto-hide delay, animations, bar width and style, accent color (automatic,
  system or a preset), music
  background and cover shape. Stored in `~/.config/dplayer/config.toml` and
  applied live to open players.

## Usage

```sh
dplayer [FILE|DIRECTORY ...]
dplayer --settings
```

Directories are expanded to their playable files in sorted order; with no
arguments the player opens on a welcome screen.

## Architecture

All decoding is delegated to `ffmpeg`/`ffprobe` **subprocesses** — the
binary has no codec dependencies and any ffmpeg build supports every
format:

```
ffprobe ── JSON metadata (streams, duration, tags)
ffmpeg  ── video: -f rawvideo rgba  → fd 3 → ImageData → creamui Image
        └─ audio: -f f32le 48kHz    → fd 4 → rodio Player → default device
```

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

Runtime requirements: `ffmpeg`, `ffprobe`, and a working audio output
(ALSA/PipeWire/PulseAudio via `rodio`). Linux builds link `libdbus-1` for
MPRIS (`dbus-devel` / `libdbus-1-dev` at build time).

## Roadmap

- Drag & drop onto the window
- Subtitle and audio track selection
- Embedded picture display while seeking
