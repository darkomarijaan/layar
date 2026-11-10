# Layar

A fast, small video player for Windows, written in Rust. [egui](https://github.com/emilk/egui) draws the interface and [libmpv](https://mpv.io) decodes and plays, so it handles nearly any format with hardware decoding.

## Download

Grab the portable package from the [Releases](https://github.com/darkomarijaan/layar/releases) page, unpack it anywhere and run `Layar.exe`. No installer. Keep `libmpv-2.dll` next to the exe.

Two formats of the same package: `.zip` (about 50 MB, opens everywhere) and `.7z` (about 33 MB, needs 7-Zip or a recent Windows 11). Most of the size is `libmpv-2.dll`, the player core; Layar itself is about 4 MB.

Requires Windows 10 or 11 (64-bit) and a GPU with OpenGL 3.3 or newer.

## Features

- Plays video and audio files, network streams (HTTP, RTSP, ...), with drag and drop and command line arguments
- Hardware decoding, exact seeking, speed 0.25x to 4x, frame stepping, A-B loop, repeat and shuffle
- Playlist, chapters list, file info panel, screenshots, and resume where you left off
- 10-band equalizer with presets, volume normalizer, compressor
- Video adjustments (brightness, contrast, saturation, gamma, hue), sharpen, denoise, aspect ratio, rotate, zoom, deinterlace
- **Subtitles**
  - tracks, external subtitle files, delay
  - a **second subtitle** shown at the same time (handy for language learning)
  - **style**: size, colour, outline, bold, position, optionally applied to styled (ASS/SSA) subtitles
  - **preferred subtitle and audio languages**, and a choice of which subtitle files to load automatically
  - **download subtitles** from OpenSubtitles.com, matched by file hash first, then by title
- **Six skins** with completely different layouts, buttons and colours (see below)
- **Auto update** from GitHub Releases (see below)
- Portable mode: settings stay in a `data` folder beside the exe

## Keys

| Key | Action |
|---|---|
| Space | play / pause |
| Left / Right | seek 5 s (Shift 1 s, Ctrl 30 s) |
| Up / Down | volume |
| M | mute |
| F / Esc | fullscreen / leave |
| P | toggle playlist |
| N / B | next / previous |
| `.` / `,` | next / previous frame |
| `[` / `]` | slower / faster |
| G / H | subtitle delay -0.1 s / +0.1 s |
| S | screenshot (saved to Pictures) |
| Ctrl+O | open files |
| F1 | shortcut help |

Click the video to play or pause, double-click for fullscreen. Playback position is remembered per file.

## Subtitles

Everything is in the **Subtitle** menu.

- **Second subtitle** picks another track to show alongside the main one.
- **Style and languages...** changes how subtitles look (live) and sets the preferred languages and auto-load mode. Language and auto-load settings apply when the next file opens. Use both 2- and 3-letter codes (`id,ind,en,eng`), because files tag languages either way.
- **Download subtitles...** searches OpenSubtitles.com. You need your own free API key (opensubtitles.com, Consumers section). Searching needs only the key; downloading also needs your account username and password. They are stored as plain text in `opensubtitles.txt` in the data folder, so do not share that folder. Downloaded files are saved next to the video (for example `Movie.en.srt`) and loaded straight away.

## Skins

Pick one in **View > Skin** (or the menu popup in the skins that have no menu bar). The choice is remembered. A skin changes the whole window, not just the colours; playback is never touched.

| Skin | Look | Layout |
|---|---|---|
| **Graphite** | Cool dark grey, blue accent (the original) | Menu bar, side panel, floating control dock |
| **Cinema** | Black and glass, ice-blue accent | Video fills the window; controls float and fade out when the mouse rests. Playlist drawer and menu behind two round buttons |
| **Deck** | Gunmetal with green LCD, like a hi-fi stack | One dense panel: bevelled screen, LCD clock and info, LCD playlist, pressable square keys, LED switches |
| **Studio** | Light, sharp corners, blue accent | Icon rail on the left, drop-downs for subtitle and audio, chapter timeline, labelled buttons |
| **Lounge** | Plum and pink, big and clear for the sofa | Poster-style screen with large title and pill buttons; playlist as a strip of cards with video thumbnails |
| **Terminal** | Amber on black, all monospace | Boxed text panes, block progress bar, keycap buttons, status line |

Fullscreen always uses the Cinema overlay, whatever skin is chosen.

Lounge cards show a real frame from each video (taken at 12 % of the file, made by libmpv itself, so no extra software is needed). They are made in the background only for cards you can see, one at a time, and cached as small JPEGs in the `thumbs` folder inside the data folder, so a file is decoded once. Audio files with cover art use the cover; files that cannot give a picture keep a colour and a number. The `thumbs` folder is safe to delete.

## Updates

At startup Layar asks GitHub whether a newer release exists (one small request, nothing about you or your files is sent). If there is, a dialog shows the release notes and offers **Update and restart**, **Later** or **Skip this version**. You can also use **Help > Check for updates...**, and turn the startup check off in the same menu.

An update downloads a small zip that holds only `Layar.exe` (about 4 MB; the full package is about 50 MB because of `libmpv-2.dll`), checks it against the SHA-256 that GitHub publishes for that file, and replaces `Layar.exe` (the previous one is kept as `Layar.exe.old` until the next start). If the checksum does not match, nothing is changed. `libmpv-2.dll` is not replaced: when a release says the player core changed, download the full package manually. If Layar sits in a folder it cannot write to (for example Program Files), the update reports an error and you can download the package yourself.

To make a release that auto-update can use, publish a GitHub release tagged `vX.Y.Z` and raise `version` in `Cargo.toml` to match. Attach:

| Asset name ends in | Contains | Used by |
|---|---|---|
| `win64-exe.zip` | `Layar.exe` only (about 4 MB) | auto-update, preferred |
| `win64-portable.zip` | the full portable folder (about 50 MB) | auto-update from 0.3.1 and earlier, and manual download |
| `win64-portable.7z` | the same folder, 7-Zip/LZMA (about 33 MB) | manual download, needs 7-Zip or a recent Windows 11 |

If a release has no `win64-exe.zip` (or it has no published checksum), the updater falls back to the full zip.

## Data folder

Settings and resume positions live in `%APPDATA%\Layar`, or in `data\` beside the exe when `portable.txt` is present (the release zip includes it). Delete `portable.txt` to use `%APPDATA%` instead.

## Build from source

You need Rust (stable, 2024 edition) and the MSVC toolchain.

1. Download `mpv-dev-x86_64-*.7z` from https://github.com/shinchiro/mpv-winbuild-cmake/releases
2. Extract it into `libmpv/` so you have `libmpv/libmpv-2.dll` and `libmpv/libmpv.dll.a`.
3. Copy `libmpv/libmpv.dll.a` to `libmpv/mpv.lib` (the MSVC linker looks for `mpv.lib`).
4. `cargo run --release -- path\to\video.mkv` (the program is `Layar.exe`)

`build.rs` links `mpv.lib` and copies `libmpv-2.dll` next to the built exe.

## Layout

| File | What it does |
|---|---|
| `src/main.rs` | interface, menus, settings, shortcuts |
| `src/video.rs` | mpv setup, OpenGL rendering into egui, data folder |
| `src/skins.rs` | the five extra skins: palettes, drawing helpers, one layout function each |
| `src/thumbs.rs` | video thumbnails through a second, short-lived libmpv instance, with a disk cache |
| `src/opensub.rs` | OpenSubtitles.com client (runs on a worker thread) |
| `src/update.rs` | GitHub release check, verified download and exe swap |

## License

Layar's own code is released under the [MIT License](LICENSE).

## Third-party software

libmpv and FFmpeg (GPL/LGPL depending on the build) are not covered by that license: the release zip ships `libmpv-2.dll` built by shinchiro, and its terms apply to that file. egui is MIT or Apache-2.0, and the Inter and JetBrains Mono fonts are SIL OFL 1.1. The release zip includes `THIRD-PARTY.txt` and a `LICENSES` folder.
