# Dplayer

A native video and music player for Linux, built on
[CreamUI](https://github.com/sammwyy/creamui).

## Features

- Video and audio playback: mp4, mkv, webm, avi, mov, mp3, flac, ogg,
  opus, m4a, wav and anything else ffmpeg understands.
- Floating transport card: seek bar (click or drag), play/pause, previous /
  next, shuffle, repeat (off → all → one), playback speed (0.5×–2×),
  volume slider and mute.
- Queue panel with add / remove / click-to-play.
- Embedded cover art for audio files, codec / resolution info line.
- Translucent blurred window, hybrid window decorations, and live system-theme
  reload.
- Keyboard-first: `space` play/pause · `←` `→` seek 5 s (`shift` = 30 s) ·
  `↑` `↓` volume · `f` maximize · `m` mute · `[` `]` speed ·
  `n`/`p` next/previous · `l` repeat · `s` shuffle · `t` queue ·
  `o` open · `q`/`Esc` quit.

## Usage

```sh
dplayer [FILE|DIRECTORY ...]
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

- `-readrate N` paces both pipes off the same input clock, so audio and
  video stay naturally coupled; playback speed adds `atempo=N`.
- Pause is `SIGSTOP` on the decoder plus `Player::pause()`; resume is
  `SIGCONT` + `play()`.
- Seek re-spawns the decoder with `-ss`. Every session is tagged with a
  generation so late frames/exits from a killed process are dropped.

## Install

```sh
cargo build --release
install -Dm755 target/release/dplayer /usr/bin/dplayer
install -Dm644 data/dplayer.desktop /usr/share/applications/dplayer.desktop
```

Runtime requirements: `ffmpeg`, `ffprobe`, and a working audio output
(ALSA/PipeWire/PulseAudio via `rodio`).

## Roadmap

- MPRIS media-keys integration (D-Bus)
- Drag & drop onto the window
- Subtitle and audio track selection
- Embedded picture display while seeking
