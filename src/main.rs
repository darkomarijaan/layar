#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(deprecated)]
mod opensub;
mod skins;
mod thumbs;
mod update;
mod video;

use skins::Skin;
use eframe::egui::{self, Color32, Key, Modifiers, RichText, Sense, Stroke, Vec2};
use std::{sync::{Arc, atomic::{AtomicU64, Ordering}, mpsc}, time::{Duration, Instant}};
use video::{Video, data_dir, migrate_legacy_data};

// Graphite Blue tokens (from the approved mockup).
const BG: Color32 = Color32::from_rgb(0x11, 0x14, 0x18);
const SURFACE: Color32 = Color32::from_rgb(0x1c, 0x21, 0x28);
const RAISED: Color32 = Color32::from_rgb(0x24, 0x2b, 0x34);
const LINE: Color32 = Color32::from_rgb(0x2b, 0x32, 0x3c);
const FG: Color32 = Color32::from_rgb(0xe6, 0xed, 0xf3);
const MUTED: Color32 = Color32::from_rgb(0x7d, 0x85, 0x90);
const ACCENT: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xff);

const EXTS: &[&str] = &["mkv", "mp4", "avi", "mov", "webm", "flv", "wmv", "ts", "m4v", "mpg", "mpeg", "mp3", "flac", "ogg", "wav", "m4a", "opus"];
const APP_NAME: &str = "Layar";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const SHORTCUTS: [(&str, &str); 22] = [
    ("Space", "Play / pause"),
    ("← / →", "Seek 5 s"),
    ("Shift + ← / →", "Seek 1 s"),
    ("Ctrl + ← / →", "Seek 30 s"),
    ("↑ / ↓", "Volume +5 / −5"),
    ("M", "Mute"),
    ("F, double-click", "Fullscreen"),
    ("Esc", "Leave fullscreen"),
    ("P", "Show / hide playlist"),
    ("N / B", "Next / previous item"),
    (". / ,", "Next / previous frame"),
    ("[ / ]", "Slower / faster"),
    ("G / H", "Subtitle delay −0.1 s / +0.1 s"),
    ("S", "Screenshot (Pictures folder)"),
    ("Ctrl + O", "Open files"),
    ("F1", "This help"),
    ("Click on video", "Play / pause"),
    ("Drop files", "Play them (first replaces the playlist)"),
    ("Audio > Equalizer", "10-band equalizer, normalizer, compressor"),
    ("Video > Adjustments", "Brightness, contrast, sharpen, denoise"),
    ("Subtitle menu", "Tracks, second subtitle, style, languages, download"),
    ("Playlist tabs", "Playlist, chapters, file info"),
];
const EQ_HZ: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
const EQ_LABELS: [&str; 10] = ["31", "62", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];
const EQ_PRESETS: [(&str, [i32; 10]); 5] = [
    ("Flat", [0; 10]),
    ("Bass", [6, 5, 4, 2, 0, 0, 0, 0, 0, 0]),
    ("Treble", [0, 0, 0, 0, 0, 1, 2, 4, 5, 6]),
    ("Vocal", [-2, -2, 0, 2, 4, 4, 3, 1, 0, -1]),
    ("Rock", [5, 4, 2, -1, -2, -1, 2, 4, 5, 5]),
];
const FX: [(&str, &str); 5] = [("Brightness", "brightness"), ("Contrast", "contrast"), ("Saturation", "saturation"), ("Gamma", "gamma"), ("Hue", "hue")];
const SUB_COLORS: [(&str, &str); 5] = [("White", "#FFFFFF"), ("Yellow", "#FFFF00"), ("Cyan", "#00FFFF"), ("Green", "#7CFC00"), ("Orange", "#FFA500")];
const SUB_AUTO: [(&str, &str); 4] = [("Off", "no"), ("Same name only", "exact"), ("Similar name", "fuzzy"), ("Every file in folder", "all")];
const SPEEDS: &[f64] =&[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

#[derive(Default, Clone, Copy, PartialEq)]
enum Tab {
    #[default]
    Playlist,
    Chapters,
    Info,
}

#[derive(Default, Clone)]
struct Track {
    id: i64,
    label: String,
    selected: bool,
}

/// What the UI shows, read from mpv. Cheap values every frame, the rest twice a second.
#[derive(Default)]
struct State {
    pos: f64,
    dur: f64,
    paused: bool,
    volume: f64,
    muted: bool,
    speed: f64,
    title: String,
    loaded: bool,
    ab_loop: bool,
    playlist: Vec<String>,
    /// Full path or URL of each playlist entry, same order as `playlist`.
    playlist_paths: Vec<String>,
    plpos: i64,
    audio: Vec<Track>,
    subs: Vec<Track>,
    sub_delay: f64,
    sid: i64,
    ssid: i64,
    path: String,
    file: String,
    video_info: String,
    hwdec: String,
    drops: i64,
    cache: f64,
    chapters: Vec<(String, f64)>,
    chapter: i64,
    info: Vec<(String, String)>,
}

struct App {
    video: Option<Video>,
    err: Option<String>,
    s: State,
    last_slow: Instant,
    show_playlist: bool,
    fullscreen: bool,
    last_move: Instant,
    url_open: bool,
    url_focus: bool,
    url: String,
    seeking: bool,
    skin: Skin,
    skin_applied: Option<Skin>,
    /// Where subtitles sit (mpv `sub-pos`, 100 = bottom) before the user's own lift; set by each layout.
    sub_base: f64,
    sub_pos_applied: f64,
    last_trim: Instant,
    trim_at: Option<Instant>,
    was_loaded: bool,
    tab: Tab,
    eq_open: bool,
    eq_on: bool,
    eq: [i32; 10],
    norm: bool,
    comp: bool,
    fx_open: bool,
    fx: [i64; 5],
    sharpen: bool,
    denoise: bool,
    af_dirty: Option<Instant>,
    help_open: bool,
    about_open: bool,
    // Subtitle appearance and loading (persisted in settings.txt).
    sub_open: bool,
    sub_scale: f64,
    sub_color: usize,
    sub_border: f64,
    sub_bold: bool,
    sub_lift: i64,
    sub_ass: bool,
    sub_auto: usize,
    slang: String,
    alang: String,
    // OpenSubtitles download window.
    os_open: bool,
    os_creds: opensub::Creds,
    os_lang: String,
    os_rx: Option<mpsc::Receiver<opensub::Msg>>,
    os_hits: Vec<opensub::Hit>,
    os_status: String,
    // Updates from GitHub Releases.
    upd_auto: bool,
    upd_skip: String,
    upd_open: bool,
    upd_manual: bool,
    upd_installing: bool,
    upd_status: String,
    upd_rel: Option<update::Release>,
    upd_rx: Option<mpsc::Receiver<update::Msg>>,
    upd_progress: Arc<AtomicU64>,
    // Lounge thumbnails, keyed by full path.
    thumbs: std::collections::HashMap<String, skins::ThumbState>,
    thumb_rx: Option<mpsc::Receiver<(String, Result<thumbs::Thumb, String>)>>,
    /// Entries whose cards were on screen last frame, nearest first; the worker takes them in this order.
    thumb_want: Vec<String>,
}


#[derive(Clone, Copy)]
enum Ic { Prev, Next, Play, Pause, Stop, Step, Full, Shot, Vol, Mute }

/// Icon button with painted glyphs (egui's bundled fonts lack the media symbols).
fn icon_btn(ui: &mut egui::Ui, ic: Ic, tip: &str, size: f32, accent: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let p = ui.painter();
    if accent {
        p.circle_filled(rect.center(), size / 2.0, if resp.hovered() { Color32::from_rgb(0x6a, 0xa0, 0xff) } else { ACCENT });
    } else if resp.hovered() {
        p.rect_filled(rect, 6.0, RAISED);
    }
    let c = if accent { Color32::WHITE } else { FG };
    let (m, u) = (rect.center(), size * 0.27);
    let pt = |x: f32, y: f32| egui::pos2(m.x + x * u, m.y + y * u);
    let tri = |dir: f32, x: f32| p.add(egui::Shape::convex_polygon(vec![pt(x - dir * 0.8, -1.0), pt(x - dir * 0.8, 1.0), pt(x + dir * 0.9, 0.0)], c, Stroke::NONE));
    let bar = |x: f32| p.rect_filled(egui::Rect::from_center_size(pt(x, 0.0), Vec2::new(u * 0.35, u * 2.0)), 1.0, c);
    let st = Stroke::new(1.8, c);
    match ic {
        Ic::Prev => { bar(-1.0); tri(-1.0, 0.5); }
        Ic::Next => { tri(1.0, -0.5); bar(1.0); }
        Ic::Play => { tri(1.0, 0.1); }
        Ic::Pause => { bar(-0.6); bar(0.6); }
        Ic::Stop => { p.rect_filled(egui::Rect::from_center_size(m, Vec2::splat(u * 1.8)), 2.0, c); }
        Ic::Step => {
            p.rect_stroke(egui::Rect::from_center_size(pt(-0.3, 0.0), Vec2::new(u * 2.0, u * 1.6)), 2.0, st, egui::StrokeKind::Inside);
            p.add(egui::Shape::convex_polygon(vec![pt(-0.8, -0.5), pt(-0.8, 0.5), pt(0.1, 0.0)], c, Stroke::NONE));
            bar(1.7);
        }
        Ic::Full => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                p.line_segment([pt(sx * 1.2, sy * 1.2), pt(sx * 1.2, sy * 0.4)], st);
                p.line_segment([pt(sx * 1.2, sy * 1.2), pt(sx * 0.4, sy * 1.2)], st);
            }
        }
        Ic::Shot => {
            p.rect_stroke(egui::Rect::from_center_size(pt(0.0, 0.2), Vec2::new(u * 2.6, u * 1.9)), 3.0, st, egui::StrokeKind::Inside);
            p.rect_filled(egui::Rect::from_center_size(pt(0.0, -0.95), Vec2::new(u * 1.0, u * 0.4)), 1.0, c);
            p.circle_stroke(pt(0.0, 0.2), u * 0.55, st);
        }
        Ic::Vol | Ic::Mute => {
            p.add(egui::Shape::convex_polygon(vec![pt(-1.4, -0.5), pt(-0.7, -0.5), pt(0.1, -1.1), pt(0.1, 1.1), pt(-0.7, 0.5), pt(-1.4, 0.5)], c, Stroke::NONE));
            if matches!(ic, Ic::Vol) {
                p.line_segment([pt(0.6, -0.5), pt(0.9, 0.0)], st);
                p.line_segment([pt(0.9, 0.0), pt(0.6, 0.5)], st);
                p.line_segment([pt(1.1, -1.0), pt(1.6, 0.0)], st);
                p.line_segment([pt(1.6, 0.0), pt(1.1, 1.0)], st);
            } else {
                p.line_segment([pt(0.6, -0.6), pt(1.6, 0.6)], st);
                p.line_segment([pt(0.6, 0.6), pt(1.6, -0.6)], st);
            }
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// Hand the pages touched only during start-up (driver, shader compile, decoder init) back to Windows.
/// Hot pages fault back in on demand. This lowers the working set Task Manager shows, not committed memory.
fn trim_memory() {
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32EmptyWorkingSet(process: isize) -> i32;
        }
        K32EmptyWorkingSet(GetCurrentProcess());
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Span {
    Plain,
    Bold,
    Code,
}

/// Split one line of Markdown into styled pieces. Handles `**bold**` and `` `code` ``; the markers are dropped.
fn md_spans(line: &str) -> Vec<(String, Span)> {
    fn flush(buf: &mut String, bold: bool, code: bool, out: &mut Vec<(String, Span)>) {
        if !buf.is_empty() {
            out.push((std::mem::take(buf), if code { Span::Code } else if bold { Span::Bold } else { Span::Plain }));
        }
    }
    let (mut out, mut buf, mut bold, mut code) = (Vec::new(), String::new(), false, false);
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        if c == '*' && it.peek() == Some(&'*') && !code {
            it.next();
            flush(&mut buf, bold, code, &mut out);
            bold = !bold;
        } else if c == '`' {
            flush(&mut buf, bold, code, &mut out);
            code = !code;
        } else {
            buf.push(c);
        }
    }
    flush(&mut buf, bold, code, &mut out);
    out
}

/// Show GitHub release notes without the raw Markdown marks: headings, bullets, bold and code.
fn release_notes(ui: &mut egui::Ui, text: &str) {
    // Some editors save a byte-order mark first; it is not whitespace, so drop it explicitly.
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let line = raw.trim();
        if line.is_empty() {
            ui.add_space(4.0);
            continue;
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if hashes > 0 && line[hashes..].starts_with(' ') {
            ui.add_space(2.0);
            ui.label(RichText::new(line[hashes..].trim()).strong().size(if hashes <= 2 { 15.0 } else { 13.5 }));
            continue;
        }
        let (bullet, body) = match line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")) {
            Some(b) => (true, b),
            None => (false, line),
        };
        let mut job = egui::text::LayoutJob::default();
        for (txt, span) in md_spans(body) {
            let (font_id, color) = match span {
                Span::Plain => (egui::FontId::proportional(12.5), Color32::from_rgb(0xb4, 0xbc, 0xc6)),
                Span::Bold => (egui::FontId::proportional(12.5), Color32::WHITE),
                Span::Code => (egui::FontId::monospace(12.0), ACCENT),
            };
            job.append(&txt, 0.0, egui::text::TextFormat { font_id, color, ..Default::default() });
        }
        if bullet {
            ui.horizontal_top(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("•").color(MUTED));
                ui.add(egui::Label::new(job).wrap());
            });
        } else {
            ui.add(egui::Label::new(job).wrap());
        }
    }
}

#[cfg(test)]
mod md_tests {
    use super::*;

    #[test]
    fn spans_drop_markers() {
        assert_eq!(md_spans("**Cinema**: video"), vec![("Cinema".into(), Span::Bold), (": video".into(), Span::Plain)]);
        assert_eq!(md_spans("keep `libmpv-2.dll` here"), vec![("keep ".into(), Span::Plain), ("libmpv-2.dll".into(), Span::Code), (" here".into(), Span::Plain)]);
        assert_eq!(md_spans("plain"), vec![("plain".into(), Span::Plain)]);
        assert!(md_spans("").is_empty());
        // An unclosed marker must not swallow the text.
        assert_eq!(md_spans("a **b"), vec![("a ".into(), Span::Plain), ("b".into(), Span::Bold)]);
    }
}

/// Settings files are line based: keep a value on one line.
fn one_line(s: &str) -> String {
    s.replace(['\r', '\n'], " ")
}

fn base_name(p: &str) -> String {
    p.rsplit(['\\', '/']).next().unwrap_or(p).to_string()
}

fn fmt_time(t: f64) -> String {
    let t = t.max(0.0) as u64;
    format!("{}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
}

fn fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    f.font_data.insert("inter".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/Inter-Regular.ttf"))));
    f.font_data.insert("jbmono".into(), Arc::new(egui::FontData::from_static(include_bytes!("../assets/JetBrainsMono-Regular.ttf"))));
    f.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "inter".into());
    f.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "jbmono".into());
    ctx.set_fonts(f);
}

fn style(ctx: &egui::Context) {
    fonts(ctx);
    ctx.set_visuals(graphite_visuals());
}

fn graphite_visuals() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = BG;
    v.faint_bg_color = SURFACE;
    v.override_text_color = Some(FG);
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    let r = egui::CornerRadius::same(6);
    for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = RAISED;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.bg_fill = RAISED;
    v.widgets.hovered.weak_bg_fill = RAISED;
    v.widgets.active.bg_fill = ACCENT;
    v.widgets.active.weak_bg_fill = RAISED;
    v
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style(&cc.egui_ctx);
        let (video, err) = match Video::new(cc) {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(e)),
        };
        let mut app = Self {
            video,
            err,
            s: State::default(),
            last_slow: Instant::now() - Duration::from_secs(1),
            show_playlist: true,
            fullscreen: false,
            last_move: Instant::now(),
            url_open: false,
            url_focus: false,
            url: String::new(),
            seeking: false,
            skin: Skin::Graphite,
            skin_applied: None,
            sub_base: 100.0,
            sub_pos_applied: f64::NAN,
            last_trim: Instant::now() - Duration::from_secs(17), // first trim ~3 s after start
            trim_at: None,
            was_loaded: false,
            tab: Tab::Playlist,
            eq_open: false,
            eq_on: true,
            eq: [0; 10],
            norm: false,
            comp: false,
            fx_open: false,
            fx: [0; 5],
            sharpen: false,
            denoise: false,
            af_dirty: None,
            help_open: false,
            about_open: false,
            sub_open: false,
            sub_scale: 1.0,
            sub_color: 0,
            sub_border: 3.0,
            sub_bold: false,
            sub_lift: 0,
            sub_ass: false,
            sub_auto: 2,
            slang: String::new(),
            alang: String::new(),
            os_open: false,
            os_creds: opensub::Creds { key: String::new(), user: String::new(), pass: String::new() },
            os_lang: "en".into(),
            os_rx: None,
            os_hits: Vec::new(),
            os_status: String::new(),
            upd_auto: true,
            upd_skip: String::new(),
            upd_open: false,
            upd_manual: false,
            upd_installing: false,
            upd_status: String::new(),
            upd_rel: None,
            upd_rx: None,
            upd_progress: Arc::new(AtomicU64::new(0)),
            thumbs: Default::default(),
            thumb_rx: None,
            thumb_want: Vec::new(),
        };
        update::cleanup(); // remove the exe an earlier update left behind
        app.load_settings();
        app.load_os_creds();
        if app.upd_auto {
            app.check_update(false);
        }
        let args: Vec<String> = std::env::args().skip(1).collect();
        app.open(&args, true);
        app
    }

    /// Replace the playlist with `paths`, or append to it.
    fn open(&mut self, paths: &[String], replace: bool) {
        let Some(v) = self.video.clone() else { return };
        for (i, p) in paths.iter().enumerate() {
            let mode = if replace && i == 0 { "replace" } else { "append-play" };
            v.cmd("loadfile", &[p, mode]);
        }
    }

    fn pick_files(&mut self, replace: bool) {
        if let Some(files) = rfd::FileDialog::new().add_filter("Media", EXTS).add_filter("All files", &["*"]).pick_files() {
            let paths: Vec<String> = files.iter().map(|f| f.to_string_lossy().into_owned()).collect();
            self.open(&paths, replace);
        }
    }

    fn refresh(&mut self) {
        let Some(v) = self.video.clone() else { return };
        let s = &mut self.s;
        s.loaded = !v.bool("idle-active");
        s.pos = v.f64("time-pos");
        s.dur = v.f64("duration");
        s.paused = v.bool("pause");
        s.volume = v.f64("volume");
        s.muted = v.bool("mute");
        s.speed = v.f64("speed");
        if self.last_slow.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.last_slow = Instant::now();
        s.title = v.str("media-title");
        s.ab_loop = v.str("ab-loop-a") != "no";
        s.sub_delay = v.f64("sub-delay");
        s.sid = v.i64("sid");
        s.ssid = v.i64("secondary-sid");
        s.path = v.str("path");
        s.file = v.str("filename");
        s.hwdec = v.str("hwdec-current");
        s.drops = v.i64("frame-drop-count");
        s.cache = v.f64("demuxer-cache-duration");
        s.plpos = v.i64("playlist-pos");
        s.playlist_paths = (0..v.i64("playlist-count")).map(|i| v.str(&format!("playlist/{i}/filename"))).collect();
        s.playlist = s.playlist_paths.iter().map(|p| base_name(p)).collect();
        let (w, h) = (v.i64("video-params/w"), v.i64("video-params/h"));
        s.video_info = if w > 0 {
            format!("{} · {}×{} · {:.3} fps", v.str("video-codec").split(' ').next().unwrap_or(""), w, h, v.f64("container-fps"))
        } else {
            String::new()
        };
        s.chapter = v.i64("chapter");
        s.chapters = (0..v.i64("chapter-list/count"))
            .map(|i| {
                let t = v.str(&format!("chapter-list/{i}/title"));
                (if t.is_empty() { format!("Chapter {}", i + 1) } else { t }, v.f64(&format!("chapter-list/{i}/time")))
            })
            .collect();
        let mut info: Vec<(String, String)> = Vec::new();
        let mut add = |k: &str, val: String| if !val.is_empty() { info.push((k.to_string(), val)) };
        add("File", v.str("filename"));
        add("Format", v.str("file-format"));
        let size = v.i64("file-size");
        add("Size", if size > 0 { format!("{:.1} MB", size as f64 / 1_048_576.0) } else { String::new() });
        add("Duration", if s.dur > 0.0 { fmt_time(s.dur) } else { String::new() });
        add("Video", s.video_info.clone());
        add("Pixel format", v.str("video-params/pixelformat"));
        add("Decoder", v.str("hwdec-current"));
        let (ch, rate) = (v.str("audio-params/hr-channels"), v.i64("audio-params/samplerate"));
        add("Audio", if ch.is_empty() { String::new() } else { format!("{} · {} · {} Hz", v.str("audio-codec-name"), ch, rate) });
        for (label, k) in [("Title", "title"), ("Artist", "artist"), ("Album", "album"), ("Date", "date")] {
            add(label, v.str(&format!("metadata/by-key/{k}")));
        }
        s.info = info;
        s.audio.clear();
        s.subs.clear();
        for i in 0..v.i64("track-list/count") {
            let key = |k: &str| format!("track-list/{i}/{k}");
            let list = match v.str(&key("type")).as_str() {
                "audio" => &mut s.audio,
                "sub" => &mut s.subs,
                _ => continue,
            };
            let id = v.i64(&key("id"));
            let parts: Vec<String> = ["lang", "title", "codec"].iter().map(|k| v.str(&key(k))).filter(|x| !x.is_empty()).collect();
            list.push(Track { id, label: format!("{id}: {}", parts.join(" · ")), selected: v.bool(&key("selected")) });
        }
        // With a second subtitle active, "selected" is true for two tracks: mark only the primary.
        let sid = s.sid;
        for t in &mut s.subs {
            t.selected = t.id == sid;
        }
    }

    fn settings_path() -> std::path::PathBuf {
        data_dir().join("settings.txt")
    }

    /// Equalizer and video adjustments survive restarts: a tiny key=value file.
    fn save_settings(&self) {
        let list = |it: &[String]| it.join(",");
        let eq: Vec<String> = self.eq.iter().map(|g| g.to_string()).collect();
        let fx: Vec<String> = self.fx.iter().map(|g| g.to_string()).collect();
        let text = format!(
            "eq={}\neq_on={}\nnorm={}\ncomp={}\nfx={}\nsharpen={}\ndenoise={}\n\
             sub_scale={}\nsub_color={}\nsub_border={}\nsub_bold={}\nsub_lift={}\nsub_ass={}\nsub_auto={}\nslang={}\nalang={}\nos_lang={}\nupd_auto={}\nupd_skip={}\nskin={}\n",
            list(&eq), self.eq_on as u8, self.norm as u8, self.comp as u8, list(&fx), self.sharpen as u8, self.denoise as u8,
            self.sub_scale, self.sub_color, self.sub_border, self.sub_bold as u8, self.sub_lift, self.sub_ass as u8, self.sub_auto,
            one_line(&self.slang), one_line(&self.alang), one_line(&self.os_lang), self.upd_auto as u8, one_line(&self.upd_skip), self.skin.key()
        );
        let path = Self::settings_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }

    fn load_settings(&mut self) {
        let Ok(text) = std::fs::read_to_string(Self::settings_path()) else { return };
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let nums = || v.split(',').filter_map(|x| x.trim().parse::<i64>().ok()).collect::<Vec<_>>();
            match k {
                "eq" => { let n = nums(); if n.len() == 10 { for (d, x) in self.eq.iter_mut().zip(n) { *d = x.clamp(-12, 12) as i32 } } }
                "fx" => { let n = nums(); if n.len() == 5 { for (d, x) in self.fx.iter_mut().zip(n) { *d = x.clamp(-100, 100) } } }
                "eq_on" => self.eq_on = v == "1",
                "norm" => self.norm = v == "1",
                "comp" => self.comp = v == "1",
                "sharpen" => self.sharpen = v == "1",
                "denoise" => self.denoise = v == "1",
                "sub_scale" => if let Ok(x) = v.parse::<f64>() { self.sub_scale = x.clamp(0.3, 4.0) },
                "sub_color" => if let Ok(x) = v.parse::<usize>() { self.sub_color = x.min(SUB_COLORS.len() - 1) },
                "sub_border" => if let Ok(x) = v.parse::<f64>() { self.sub_border = x.clamp(0.0, 8.0) },
                "sub_bold" => self.sub_bold = v == "1",
                "sub_lift" => if let Ok(x) = v.parse::<i64>() { self.sub_lift = x.clamp(0, 60) },
                "sub_ass" => self.sub_ass = v == "1",
                "sub_auto" => if let Ok(x) = v.parse::<usize>() { self.sub_auto = x.min(SUB_AUTO.len() - 1) },
                "slang" => self.slang = v.to_string(),
                "alang" => self.alang = v.to_string(),
                "os_lang" => self.os_lang = v.to_string(),
                "upd_auto" => self.upd_auto = v == "1",
                "upd_skip" => self.upd_skip = v.to_string(),
                "skin" => if let Some(s) = Skin::from_key(v) { self.skin = s },
                _ => {}
            }
        }
        self.apply_af();
        self.apply_vf();
        self.apply_sub();
        if let Some(v) = &self.video {
            for (i, (_, prop)) in FX.iter().enumerate() {
                v.set_i64(prop, self.fx[i]);
            }
        }
    }

    /// Push subtitle look, language preferences and auto-load mode to mpv.
    /// Languages and auto-load take effect when the next file opens.
    fn apply_sub(&mut self) {
        let Some(v) = &self.video else { return };
        v.set_f64("sub-scale", self.sub_scale);
        v.set_str("sub-color", SUB_COLORS[self.sub_color].1);
        v.set_f64("sub-border-size", self.sub_border);
        v.set_bool("sub-bold", self.sub_bold);
        // "force" restyles ASS/SSA subtitles too; "scale" keeps their own styling but honours the size.
        v.set_str("sub-ass-override", if self.sub_ass { "force" } else { "scale" });
        v.set_str("sub-auto", SUB_AUTO[self.sub_auto].1);
        v.set_str("sub-file-paths", "sub;subs;Subs;Subtitles");
        v.set_str("slang", self.slang.trim());
        v.set_str("alang", self.alang.trim());
        self.sub_pos_applied = f64::NAN; // re-applies sub-pos with the new lift
    }

    /// Ask GitHub for the latest release in the background. A manual check reports every outcome;
    /// the automatic one stays silent unless a newer version exists.
    fn check_update(&mut self, manual: bool) {
        if self.upd_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.upd_rx = Some(rx);
        self.upd_manual = manual;
        if manual {
            self.upd_open = true;
            self.upd_rel = None;
            self.upd_status = "Checking for updates…".into();
        }
        std::thread::spawn(move || {
            let _ = tx.send(match update::check(update::REPO, VERSION) {
                Ok(r) => update::Msg::Checked(r),
                Err(e) => update::Msg::Error(e),
            });
        });
    }

    fn install_update(&mut self) {
        let (Some(rel), None) = (self.upd_rel.clone(), &self.upd_rx) else { return };
        let Ok(exe) = std::env::current_exe() else { return };
        let (tx, rx) = mpsc::channel();
        self.upd_rx = Some(rx);
        self.upd_installing = true;
        self.upd_status = "Downloading…".into();
        self.upd_progress.store(0, Ordering::Relaxed);
        let progress = self.upd_progress.clone();
        std::thread::spawn(move || {
            let _ = tx.send(match update::install(&rel, &exe, &progress) {
                Ok(()) => update::Msg::Installed,
                Err(e) => update::Msg::Error(e),
            });
        });
    }

    fn upd_poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.upd_rx else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => update::Msg::Error("Update worker stopped unexpectedly".into()),
        };
        self.upd_rx = None;
        let was_installing = std::mem::take(&mut self.upd_installing);
        match msg {
            update::Msg::Checked(Some(rel)) => {
                if self.upd_manual || rel.version != self.upd_skip {
                    self.upd_status.clear();
                    self.upd_rel = Some(rel);
                    self.upd_open = true;
                } else {
                    self.upd_open = false;
                }
            }
            update::Msg::Checked(None) => {
                if self.upd_manual {
                    self.upd_status = format!("You have the latest version ({VERSION}).");
                }
            }
            update::Msg::Installed => {
                self.save_settings();
                if let Some(v) = &self.video {
                    v.cmd("write-watch-later-config", &[]);
                }
                match update::restart() {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.upd_status = format!("Updated, but could not restart: {e}. Start Layar again."),
                }
            }
            update::Msg::Error(e) => {
                if self.upd_manual || was_installing {
                    self.upd_status = format!("Update failed: {e}");
                    self.upd_open = true;
                }
            }
        }
    }

    fn os_path() -> std::path::PathBuf {
        data_dir().join("opensubtitles.txt")
    }

    /// API key and login live in their own file, apart from the settings you might share.
    fn load_os_creds(&mut self) {
        let Ok(text) = std::fs::read_to_string(Self::os_path()) else { return };
        for line in text.lines() {
            match line.split_once('=') {
                Some(("key", v)) => self.os_creds.key = v.to_string(),
                Some(("user", v)) => self.os_creds.user = v.to_string(),
                Some(("pass", v)) => self.os_creds.pass = v.to_string(),
                _ => {}
            }
        }
    }

    fn save_os_creds(&self) {
        let c = &self.os_creds;
        let _ = std::fs::create_dir_all(data_dir());
        let _ = std::fs::write(Self::os_path(), format!("key={}\nuser={}\npass={}\n", one_line(&c.key), one_line(&c.user), one_line(&c.pass)));
    }

    fn os_busy(&self) -> bool {
        self.os_rx.is_some()
    }

    fn os_search(&mut self) {
        self.save_os_creds();
        self.save_settings();
        let (tx, rx) = mpsc::channel();
        let (key, lang) = (self.os_creds.key.clone(), self.os_lang.clone());
        let path = std::path::Path::new(&self.s.path).is_file().then(|| self.s.path.clone());
        let title = if self.s.file.is_empty() { self.s.title.clone() } else { self.s.file.clone() };
        self.os_hits.clear();
        self.os_status = "Searching…".into();
        self.os_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(match opensub::search(&key, &lang, path.as_deref(), &title) {
                Ok(h) => opensub::Msg::Hits(h),
                Err(e) => opensub::Msg::Error(e),
            });
        });
    }

    fn os_download(&mut self, hit: &opensub::Hit) {
        self.save_os_creds();
        let lang = if hit.lang.is_empty() { "sub".to_string() } else { hit.lang.clone() };
        let video = std::path::Path::new(&self.s.path);
        let fallback = data_dir().join("subtitles");
        let dest = if video.is_file() {
            video.with_extension(format!("{lang}.srt")) // next to the video, so auto-load finds it later too
        } else {
            let name: String = self.s.title.chars().filter(|c| !"\\/:*?\"<>|".contains(*c)).collect();
            fallback.join(format!("{}.{lang}.srt", if name.is_empty() { "subtitle" } else { &name }))
        };
        let (tx, rx) = mpsc::channel();
        let (creds, id) = (self.os_creds.clone(), hit.file_id);
        self.os_status = "Downloading…".into();
        self.os_rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(match opensub::download(&creds, id, dest, fallback) {
                Ok(p) => opensub::Msg::Saved(p),
                Err(e) => opensub::Msg::Error(e),
            });
        });
    }

    fn os_poll(&mut self) {
        let Some(rx) = &self.os_rx else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => opensub::Msg::Error("Worker stopped unexpectedly".into()),
        };
        self.os_rx = None;
        match msg {
            opensub::Msg::Hits(h) => {
                self.os_status = if h.is_empty() { "No subtitles found".into() } else { format!("{} found", h.len()) };
                self.os_hits = h;
            }
            opensub::Msg::Saved(p) => {
                if let Some(v) = &self.video {
                    v.cmd("sub-add", &[&p.to_string_lossy(), "select"]);
                }
                self.os_status = format!("Loaded {}", p.display());
            }
            opensub::Msg::Error(e) => self.os_status = format!("Error: {e}"),
        }
    }

    /// Rebuild mpv's audio filter chain from the equalizer, normalizer and compressor settings.
    fn apply_af(&self) {
        let Some(v) = &self.video else { return };
        let mut f: Vec<String> = Vec::new();
        if self.eq_on {
            for (hz, g) in EQ_HZ.iter().zip(self.eq) {
                if g != 0 {
                    f.push(format!("equalizer=f={hz}:t=o:w=1:g={g}"));
                }
            }
        }
        if self.norm {
            f.push("dynaudnorm".into());
        }
        if self.comp {
            f.push("acompressor".into());
        }
        v.set_str("af", &if f.is_empty() { String::new() } else { format!("lavfi=[{}]", f.join(",")) });
    }

    fn apply_vf(&self) {
        let Some(v) = &self.video else { return };
        let mut f: Vec<&str> = Vec::new();
        if self.sharpen {
            f.push("unsharp=5:5:1.0");
        }
        if self.denoise {
            f.push("hqdn3d");
        }
        v.set_str("vf", &if f.is_empty() { String::new() } else { format!("lavfi=[{}]", f.join(",")) });
    }

    /// Vertical bar (bottom = 0, top = 1) with the fill drawn from the middle. Returns the new fraction.
    fn vbar(ui: &mut egui::Ui, height: f32, frac: f32) -> Option<f32> {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(26.0, height), Sense::click_and_drag());
        let track = egui::Rect::from_center_size(rect.center(), Vec2::new(4.0, height - 12.0));
        let y = |f: f32| track.bottom() - track.height() * f;
        let p = ui.painter();
        p.rect_filled(track, 2.0, RAISED);
        let (a, b) = (y(frac), y(0.5));
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(track.left(), a.min(b)), egui::pos2(track.right(), a.max(b))), 2.0, ACCENT);
        p.circle_filled(egui::pos2(track.center().x, a), if resp.hovered() || resp.dragged() { 7.0 } else { 5.0 }, FG);
        if resp.clicked() || resp.dragged() {
            return resp.interact_pointer_pos().map(|pt| ((track.bottom() - pt.y) / track.height()).clamp(0.0, 1.0));
        }
        None
    }

    fn toggle_pause(&self) {
        if let Some(v) = &self.video {
            v.cmd("cycle", &["pause"]);
        }
    }

    fn seek_rel(&self, secs: f64) {
        if let Some(v) = &self.video {
            v.cmd("seek", &[&secs.to_string(), "relative"]);
        }
    }

    fn set_fullscreen(&mut self, ctx: &egui::Context, on: bool) {
        self.fullscreen = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(on));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let pressed = |k: Key, m: Modifiers| ctx.input_mut(|i| i.consume_key(m, k));
        let Some(v) = self.video.clone() else { return };
        if pressed(Key::Space, Modifiers::NONE) {
            self.toggle_pause();
        }
        // Most specific combination first: a plain-arrow check would also eat Shift+Arrow.
        for (key, sign) in [(Key::ArrowRight, 1.0), (Key::ArrowLeft, -1.0)] {
            for (mods, secs) in [(Modifiers::CTRL, 30.0), (Modifiers::SHIFT, 1.0), (Modifiers::NONE, 5.0)] {
                if pressed(key, mods) {
                    self.seek_rel(sign * secs);
                    break;
                }
            }
        }
        if pressed(Key::ArrowUp, Modifiers::NONE) { v.set_f64("volume", (self.s.volume + 5.0).min(200.0)) }
        if pressed(Key::ArrowDown, Modifiers::NONE) { v.set_f64("volume", (self.s.volume - 5.0).max(0.0)) }
        if pressed(Key::M, Modifiers::NONE) { v.cmd("cycle", &["mute"]) }
        if pressed(Key::S, Modifiers::NONE) { v.cmd("screenshot", &[]) }
        if pressed(Key::Period, Modifiers::NONE) { v.cmd("frame-step", &[]) }
        if pressed(Key::Comma, Modifiers::NONE) { v.cmd("frame-back-step", &[]) }
        if pressed(Key::OpenBracket, Modifiers::NONE) { v.set_f64("speed", (self.s.speed / 1.1).max(0.25)) }
        if pressed(Key::CloseBracket, Modifiers::NONE) { v.set_f64("speed", (self.s.speed * 1.1).min(4.0)) }
        if pressed(Key::G, Modifiers::NONE) { v.cmd("add", &["sub-delay", "-0.1"]) }
        if pressed(Key::H, Modifiers::NONE) { v.cmd("add", &["sub-delay", "0.1"]) }
        if pressed(Key::N, Modifiers::NONE) { v.cmd("playlist-next", &[]) }
        if pressed(Key::B, Modifiers::NONE) { v.cmd("playlist-prev", &[]) }
        if pressed(Key::P, Modifiers::NONE) { self.show_playlist = !self.show_playlist }
        if pressed(Key::F, Modifiers::NONE) { let on = !self.fullscreen; self.set_fullscreen(ctx, on) }
        if self.fullscreen && pressed(Key::Escape, Modifiers::NONE) { self.set_fullscreen(ctx, false) }
        if pressed(Key::O, Modifiers::COMMAND) { self.pick_files(true) }
        if pressed(Key::F1, Modifiers::NONE) { self.help_open = !self.help_open }
    }

    fn menu(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| self.menu_items(ctx, ui));
    }

    /// The top-level menus. Graphite shows them in a menu bar; the other skins put them in a popup.
    pub(crate) fn menu_items(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let Some(v) = self.video.clone() else { return };
        {
            ui.menu_button("Media", |ui| {
                if ui.button("Open files…   Ctrl+O").clicked() { self.pick_files(true); ui.close(); }
                if ui.button("Add to playlist…").clicked() { self.pick_files(false); ui.close(); }
                if ui.button("Open network stream…").clicked() { self.url_open = true; self.url_focus = true; ui.close(); }
                ui.separator();
                if ui.button("Quit").clicked() { ctx.send_viewport_cmd(egui::ViewportCommand::Close); }
            });
            ui.menu_button("Playback", |ui| {
                if ui.button("Play / pause   Space").clicked() { v.cmd("cycle", &["pause"]); ui.close(); }
                if ui.button("Stop").clicked() { v.cmd("stop", &[]); ui.close(); }
                if ui.button("Next frame   .").clicked() { v.cmd("frame-step", &[]); ui.close(); }
                if ui.button("Previous frame   ,").clicked() { v.cmd("frame-back-step", &[]); ui.close(); }
                if ui.button("Loop A–B").clicked() { v.cmd("ab-loop", &[]); ui.close(); }
                ui.menu_button("Speed", |ui| {
                    for &sp in SPEEDS {
                        if ui.selectable_label((self.s.speed - sp).abs() < 0.01, format!("{sp}×")).clicked() { v.set_f64("speed", sp); ui.close(); }
                    }
                });
                ui.menu_button("Repeat", |ui| {
                    if ui.button("Off").clicked() { v.set_str("loop-playlist", "no"); v.set_str("loop-file", "no"); ui.close(); }
                    if ui.button("Repeat file").clicked() { v.set_str("loop-file", "inf"); ui.close(); }
                    if ui.button("Repeat playlist").clicked() { v.set_str("loop-playlist", "inf"); ui.close(); }
                });
                if ui.button("Shuffle playlist").clicked() { v.cmd("playlist-shuffle", &[]); ui.close(); }
            });
            ui.menu_button("Audio", |ui| {
                if self.s.audio.is_empty() { ui.label(RichText::new("No audio tracks").color(MUTED)); }
                for t in &self.s.audio {
                    if ui.selectable_label(t.selected, &t.label).clicked() { v.set_i64("aid", t.id); ui.close(); }
                }
                ui.separator();
                if ui.button("Equalizer…").clicked() { self.eq_open = true; ui.close(); }
                if ui.button("Mute   M").clicked() { v.cmd("cycle", &["mute"]); ui.close(); }
            });
            ui.menu_button("Video", |ui| {
                ui.menu_button("Aspect ratio", |ui| {
                    for (name, val) in [("Auto", "-1"), ("16:9", "16:9"), ("4:3", "4:3"), ("21:9", "21:9"), ("1:1", "1:1")] {
                        if ui.button(name).clicked() { v.set_str("video-aspect-override", val); ui.close(); }
                    }
                });
                ui.menu_button("Rotate", |ui| {
                    for deg in [0i64, 90, 180, 270] {
                        if ui.button(format!("{deg}°")).clicked() { v.set_i64("video-rotate", deg); ui.close(); }
                    }
                });
                ui.menu_button("Zoom", |ui| {
                    for (name, z) in [("100%", 0.0), ("125%", 0.322), ("150%", 0.585), ("200%", 1.0)] {
                        if ui.button(name).clicked() { v.set_f64("video-zoom", z); ui.close(); }
                    }
                });
                ui.menu_button("Deinterlace", |ui| {
                    if ui.button("Off").clicked() { v.set_bool("deinterlace", false); ui.close(); }
                    if ui.button("On").clicked() { v.set_bool("deinterlace", true); ui.close(); }
                });
                if ui.button("Adjustments…").clicked() { self.fx_open = true; ui.close(); }
                if ui.button("Screenshot   S").clicked() { v.cmd("screenshot", &[]); ui.close(); }
                if ui.button("Fullscreen   F").clicked() { let on = !self.fullscreen; self.set_fullscreen(ctx, on); ui.close(); }
                let top = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let _ = top;
            });
            ui.menu_button("Subtitle", |ui| {
                if ui.button("Add subtitle file…").clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("Subtitles", &["srt", "ass", "ssa", "vtt", "sub"]).pick_file() {
                        v.cmd("sub-add", &[&f.to_string_lossy(), "select"]);
                    }
                    ui.close();
                }
                if ui.selectable_label(self.s.subs.iter().all(|t| !t.selected), "Disabled").clicked() { v.set_str("sid", "no"); ui.close(); }
                for t in &self.s.subs {
                    if ui.selectable_label(t.selected, &t.label).clicked() { v.set_i64("sid", t.id); ui.close(); }
                }
                ui.menu_button("Second subtitle", |ui| {
                    if ui.selectable_label(self.s.ssid <= 0, "None").clicked() { v.set_str("secondary-sid", "no"); ui.close(); }
                    for t in &self.s.subs {
                        if ui.selectable_label(t.id == self.s.ssid, &t.label).clicked() { v.set_i64("secondary-sid", t.id); ui.close(); }
                    }
                });
                ui.separator();
                if ui.button("Delay −0.1 s   G").clicked() { v.cmd("add", &["sub-delay", "-0.1"]); }
                if ui.button("Delay +0.1 s   H").clicked() { v.cmd("add", &["sub-delay", "0.1"]); }
                ui.separator();
                if ui.button("Style and languages…").clicked() { self.sub_open = true; ui.close(); }
                if ui.button("Download subtitles…").clicked() { self.os_open = true; ui.close(); }
            });
            ui.menu_button("View", |ui| {
                if ui.checkbox(&mut self.show_playlist, "Playlist   P").clicked() { ui.close(); }
                ui.menu_button("Skin", |ui| {
                    for sk in Skin::ALL {
                        if ui.selectable_label(self.skin == sk, sk.name()).clicked() { self.skin = sk; self.save_settings(); ui.close(); }
                    }
                });
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard shortcuts   F1").clicked() { self.help_open = true; ui.close(); }
                ui.separator();
                if ui.button("Check for updates…").clicked() { self.check_update(true); ui.close(); }
                if ui.checkbox(&mut self.upd_auto, "Check for updates at startup").changed() { self.save_settings(); }
                ui.separator();
                if ui.button(format!("About {APP_NAME}")).clicked() { self.about_open = true; ui.close(); }
            });
        }
    }

    /// Thin horizontal bar: click or drag to pick a position. Returns the new fraction 0..=1.
    fn bar(ui: &mut egui::Ui, width: f32, frac: f32, cached: f32, knob: f32, centered: bool) -> Option<f32> {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click_and_drag());
        let track = egui::Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 4.0));
        let p = ui.painter();
        p.rect_filled(track, 2.0, RAISED);
        let at = |f: f32| egui::pos2(track.left() + track.width() * f, track.bottom());
        if cached > 0.0 {
            p.rect_filled(egui::Rect::from_min_max(track.min, at(cached)), 2.0, Color32::from_rgb(0x34, 0x40, 0x4f));
        }
        let start: f32 = if centered { 0.5 } else { 0.0 };
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(at(start.min(frac)).x, track.top()), egui::pos2(at(start.max(frac)).x, track.bottom())), 2.0, ACCENT);
        p.circle_filled(egui::pos2(at(frac).x, track.center().y), if resp.hovered() || resp.dragged() { knob + 2.0 } else { knob }, FG);
        if resp.clicked() || resp.dragged() {
            return resp.interact_pointer_pos().map(|pt| ((pt.x - track.left()) / track.width()).clamp(0.0, 1.0));
        }
        None
    }

    /// Seek bar: returns the target time in seconds.
    fn seek_bar(&self, ui: &mut egui::Ui) -> Option<f64> {
        if self.s.dur <= 0.0 {
            Self::bar(ui, ui.available_width(), 0.0, 0.0, 5.0, false);
            return None;
        }
        let frac = (self.s.pos / self.s.dur).clamp(0.0, 1.0) as f32;
        let cached = ((self.s.pos + self.s.cache) / self.s.dur).clamp(0.0, 1.0) as f32;
        Self::bar(ui, ui.available_width(), frac, cached, 5.0, false).map(|f| f as f64 * self.s.dur)
    }

    fn dock(&mut self, ctx: &egui::Context, video_rect: egui::Rect) {
        let Some(v) = self.video.clone() else { return };
        let width = (video_rect.width() - 24.0).max(200.0);
        let s_paused = self.s.paused;
        let mut next_fullscreen = None;
        let mut seek_to = None;
        egui::Area::new(egui::Id::new("dock"))
            .order(egui::Order::Middle)
            .fixed_pos(egui::pos2(video_rect.left() + 12.0, video_rect.bottom() - 12.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ctx, |ui| {
                egui::Frame::new().fill(SURFACE.linear_multiply(0.94)).stroke(Stroke::new(1.0, LINE)).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(width - 20.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(fmt_time(self.s.pos)).monospace().color(FG));
                        let remaining = ui.available_width() - 70.0;
                        ui.allocate_ui(Vec2::new(remaining.max(50.0), 18.0), |ui| seek_to = self.seek_bar(ui));
                        ui.label(RichText::new(fmt_time(self.s.dur)).monospace().color(MUTED));
                    });
                    ui.horizontal(|ui| {
                                                if icon_btn(ui, Ic::Prev, "Previous (B)", 32.0, false) { v.cmd("playlist-prev", &[]); }
                        if icon_btn(ui, if s_paused { Ic::Play } else { Ic::Pause }, "Play / pause (Space)", 38.0, true) { v.cmd("cycle", &["pause"]); }
                        if icon_btn(ui, Ic::Next, "Next (N)", 32.0, false) { v.cmd("playlist-next", &[]); }
                        if icon_btn(ui, Ic::Stop, "Stop", 32.0, false) { v.cmd("stop", &[]); }
                        if icon_btn(ui, Ic::Step, "Next frame (.)", 32.0, false) { v.cmd("frame-step", &[]); }
                        ui.add_space(8.0);
                        let ab = ui.add(egui::Button::new(RichText::new("A–B").monospace().color(if self.s.ab_loop { ACCENT } else { FG })).fill(RAISED)).on_hover_text("Loop A–B");
                        if ab.clicked() { v.cmd("ab-loop", &[]); }
                        egui::ComboBox::from_id_salt("speed").selected_text(format!("{}×", self.s.speed)).width(64.0).show_ui(ui, |ui| {
                            for &sp in SPEEDS {
                                if ui.selectable_label((self.s.speed - sp).abs() < 0.01, format!("{sp}×")).clicked() { v.set_f64("speed", sp); }
                            }
                        });
                        let sub_txt = self.s.subs.iter().find(|t| t.selected).map_or("Sub: off".to_string(), |t| format!("Sub {}", t.id));
                        egui::ComboBox::from_id_salt("sub").selected_text(sub_txt).width(76.0).show_ui(ui, |ui| {
                            if ui.selectable_label(self.s.subs.iter().all(|t| !t.selected), "Disabled").clicked() { v.set_str("sid", "no"); }
                            for t in &self.s.subs {
                                if ui.selectable_label(t.selected, &t.label).clicked() { v.set_i64("sid", t.id); }
                            }
                        });
                        let aud_txt = self.s.audio.iter().find(|t| t.selected).map_or("Audio".to_string(), |t| format!("Audio {}", t.id));
                        egui::ComboBox::from_id_salt("aud").selected_text(aud_txt).width(76.0).show_ui(ui, |ui| {
                            for t in &self.s.audio {
                                if ui.selectable_label(t.selected, &t.label).clicked() { v.set_i64("aid", t.id); }
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_btn(ui, Ic::Full, "Fullscreen (F)", 32.0, false) { next_fullscreen = Some(!self.fullscreen); }
                            if icon_btn(ui, Ic::Shot, "Screenshot (S)", 32.0, false) { v.cmd("screenshot", &[]); }
                            if let Some(f) = Self::bar(ui, 70.0, (self.s.volume / 200.0).clamp(0.0, 1.0) as f32, 0.0, 4.0, false) { v.set_f64("volume", (f * 200.0).round() as f64); }
                            if icon_btn(ui, if self.s.muted { Ic::Mute } else { Ic::Vol }, "Mute (M)", 32.0, false) { v.cmd("cycle", &["mute"]); }
                        });
                    });
                });
            });
        if let Some(t) = seek_to {
            v.cmd("seek", &[&t.to_string(), "absolute"]);
            self.seeking = true;
        }
        if let Some(on) = next_fullscreen {
            self.set_fullscreen(ctx, on);
        }
    }

    /// One 30px list row: number, text, optional right-hand text. Returns true when clicked.
    fn list_row(ui: &mut egui::Ui, num: usize, text: &str, right: &str, now: bool) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
        if now {
            ui.painter().rect_filled(rect, 6.0, ACCENT.linear_multiply(0.18));
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 6.0, RAISED);
        }
        let p = ui.painter();
        p.text(rect.left_center() + Vec2::new(12.0, 0.0), egui::Align2::CENTER_CENTER, num.to_string(), egui::FontId::monospace(12.0), if now { ACCENT } else { MUTED });
        let right_w = if right.is_empty() { 0.0 } else { 64.0 };
        p.text(rect.right_center() + Vec2::new(-8.0, 0.0), egui::Align2::RIGHT_CENTER, right, egui::FontId::monospace(12.0), MUTED);
        let clip = ui.painter().with_clip_rect(egui::Rect::from_min_max(rect.min, egui::pos2(rect.right() - right_w, rect.bottom())));
        clip.text(rect.left_center() + Vec2::new(28.0, 0.0), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(13.0), FG);
        resp.clicked()
    }

    fn playlist_panel(&mut self, root: &mut egui::Ui) {
        let Some(v) = self.video.clone() else { return };
        egui::Panel::right("playlist").exact_size(300.0).resizable(false).frame(egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0, LINE))).show(root, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal(|ui| {
                let w = ui.available_width() / 3.0;
                for (t, name) in [(Tab::Playlist, "Playlist"), (Tab::Chapters, "Chapter"), (Tab::Info, "Info")] {
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 34.0), Sense::click());
                    let on = self.tab == t;
                    let col = if on { FG } else if resp.hovered() { FG.linear_multiply(0.8) } else { MUTED };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(13.0), col);
                    if on {
                        ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom()), 0.0, ACCENT);
                    }
                    if resp.clicked() {
                        self.tab = t;
                    }
                }
            });
            ui.separator();
            ui.spacing_mut().item_spacing.x = 8.0;
            match self.tab {
                Tab::Playlist => {
                    ui.horizontal(|ui| {
                        if ui.small_button("Add…").clicked() { self.pick_files(false); }
                        if ui.small_button("Clear").clicked() { v.cmd("stop", &[]); } // stop also empties the playlist, unlike playlist-clear
                    });
                    let mut jump = None;
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.s.playlist.is_empty() {
                            ui.add_space(12.0);
                            ui.label(RichText::new("Drop files here or press Ctrl+O").color(MUTED));
                        }
                        for (i, name) in self.s.playlist.iter().enumerate() {
                            if Self::list_row(ui, i + 1, name, "", i as i64 == self.s.plpos) { jump = Some(i); }
                        }
                    });
                    if let Some(i) = jump {
                        v.set_i64("playlist-pos", i as i64);
                    }
                }
                Tab::Chapters => {
                    let mut jump = None;
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.s.chapters.is_empty() {
                            ui.add_space(12.0);
                            ui.label(RichText::new("No chapters in this file").color(MUTED));
                        }
                        for (i, (name, t)) in self.s.chapters.iter().enumerate() {
                            if Self::list_row(ui, i + 1, name, &fmt_time(*t + 0.05), i as i64 == self.s.chapter) { jump = Some(i); }
                        }
                    });
                    if let Some(i) = jump {
                        v.set_i64("chapter", i as i64);
                    }
                }
                Tab::Info => {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.s.info.is_empty() {
                            ui.add_space(12.0);
                            ui.label(RichText::new("Nothing playing").color(MUTED));
                        }
                        ui.add_space(6.0);
                        for (k, val) in &self.s.info {
                            ui.label(RichText::new(k).size(11.0).color(MUTED));
                            ui.add(egui::Label::new(RichText::new(val).size(13.0)).wrap());
                            ui.add_space(6.0);
                        }
                    });
                }
            }
        });
    }

    fn status_bar(&self, root: &mut egui::Ui) {
        egui::Panel::bottom("status").exact_size(26.0).frame(egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(12, 4))).show(root, |ui| {
            ui.horizontal(|ui| {
                let (txt, col) = if !self.s.loaded { ("● Idle", MUTED) } else if self.s.paused { ("● Paused", MUTED) } else { ("● Playing", ACCENT) };
                ui.label(RichText::new(txt).monospace().color(col));
                let m = |s: String| RichText::new(s).monospace().color(MUTED);
                ui.add_space(12.0);
                ui.label(m(format!("hwdec {}", if self.s.hwdec.is_empty() { "-" } else { &self.s.hwdec })));
                ui.add_space(12.0);
                ui.label(m(format!("buffer {:.1} s", self.s.cache)));
                ui.add_space(12.0);
                ui.label(m(format!("drop {}", self.s.drops)));
                ui.add_space(12.0);
                ui.label(m(format!("volume {:.0}%", self.s.volume)));
            });
        });
    }
}

impl App {
    /// The original layout: menu bar on top, status bar, side panel, floating dock.
    fn layout_graphite(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if !self.fullscreen {
            egui::Panel::top("menu").frame(egui::Frame::new().fill(SURFACE).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(8, 4))).show(ui, |ui| self.menu(ctx, ui));
            self.status_bar(ui);
            if self.show_playlist {
                self.playlist_panel(ui);
            }
        }

        let mut video_rect = egui::Rect::NOTHING;
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(ui, |ui| {
            video_rect = ui.max_rect();
            if let Some(v) = &self.video {
                v.paint(ui, video_rect);
            }
            if self.s.loaded {
                ui.painter().text(video_rect.left_top() + Vec2::new(12.0, 12.0), egui::Align2::LEFT_TOP, &self.s.title, egui::FontId::proportional(14.0), FG);
                if !self.s.video_info.is_empty() {
                    ui.painter().text(video_rect.right_top() + Vec2::new(-12.0, 12.0), egui::Align2::RIGHT_TOP, &self.s.video_info, egui::FontId::monospace(12.0), MUTED);
                }
            } else {
                ui.painter().text(video_rect.center(), egui::Align2::CENTER_CENTER, "Drop a video here or press Ctrl+O", egui::FontId::proportional(18.0), MUTED);
            }
            let resp = ui.interact(video_rect, egui::Id::new("stage"), Sense::click());
            if resp.double_clicked() {
                let on = !self.fullscreen;
                self.set_fullscreen(ctx, on);
            } else if resp.clicked() {
                self.toggle_pause();
            }
        });

        // In fullscreen the dock hides after 3 s without mouse movement.
        let dock_visible = !self.fullscreen || self.last_move.elapsed() < Duration::from_secs(3) || self.s.paused;
        if dock_visible {
            self.dock(ctx, video_rect);
        }
        // The dock floats over the picture: lift subtitles above it.
        self.sub_base = if dock_visible { 84.0 } else { 100.0 };
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The mpv core is never destroyed, so its own save-on-quit never runs.
        self.save_settings();
        if let Some(v) = &self.video {
            v.cmd("write-watch-later-config", &[]);
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(100));
        if self.af_dirty.is_some_and(|t| t.elapsed() > Duration::from_millis(250)) {
            self.af_dirty = None;
            self.apply_af();
        }
        self.refresh();
        self.os_poll();
        self.upd_poll(ctx);
        self.thumb_poll(ctx);
        self.shortcuts(ctx);

        // Trim once things settle after a file opens, and every 20 s otherwise.
        if self.s.loaded && !self.was_loaded {
            self.trim_at = Some(Instant::now() + Duration::from_secs(3));
        }
        self.was_loaded = self.s.loaded;
        if self.trim_at.is_some_and(|t| Instant::now() >= t) || self.last_trim.elapsed() > Duration::from_secs(20) {
            trim_memory();
            self.last_trim = Instant::now();
            self.trim_at = None;
        }

        let dropped: Vec<String> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_string_lossy().into_owned()).collect());
        if !dropped.is_empty() {
            self.open(&dropped, true);
        }
        if ctx.input(|i| i.pointer.delta() != Vec2::ZERO || i.pointer.any_down()) {
            self.last_move = Instant::now();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        if let Some(e) = &self.err {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.heading("Cannot start mpv");
                ui.label(e);
                ui.label("Make sure libmpv-2.dll sits next to the executable.");
            });
            return;
        }

        self.apply_skin(ctx);
        // Fullscreen always uses the clean Cinema overlay, whatever the skin.
        let skin = if self.fullscreen && self.skin != Skin::Graphite { Skin::Cinema } else { self.skin };
        if skin == Skin::Graphite {
            self.layout_graphite(ui, ctx);
        } else {
            self.layout_skin(skin, ui, ctx);
        }
        // Keep subtitles clear of whatever the layout draws over the bottom of the picture.
        let want = self.sub_base - self.sub_lift as f64;
        if self.sub_pos_applied.is_nan() || (want - self.sub_pos_applied).abs() > 0.05 {
            if let Some(v) = &self.video {
                v.set_f64("sub-pos", want);
            }
            self.sub_pos_applied = want;
        }

        if self.eq_open {
            let (mut open, mut changed) = (true, false);
            egui::Window::new("Equalizer").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut self.eq_on, "Enable").changed();
                    for (name, preset) in EQ_PRESETS {
                        if ui.small_button(name).clicked() {
                            self.eq = preset;
                            changed = true;
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    for i in 0..10 {
                        ui.vertical(|ui| {
                            ui.set_width(30.0);
                            if let Some(f) = Self::vbar(ui, 130.0, (self.eq[i] as f32 + 12.0) / 24.0) {
                                self.eq[i] = (f * 24.0 - 12.0).round() as i32;
                                changed = true;
                            }
                            ui.label(RichText::new(format!("{:+}", self.eq[i])).monospace().size(11.0));
                            ui.label(RichText::new(EQ_LABELS[i]).monospace().size(11.0).color(MUTED));
                        });
                    }
                });
                ui.separator();
                changed |= ui.checkbox(&mut self.norm, "Normalize volume").changed();
                changed |= ui.checkbox(&mut self.comp, "Compressor").changed();
            });
            if changed {
                self.af_dirty = Some(Instant::now());
                self.save_settings();
            }
            self.eq_open = open;
        }

        if self.fx_open {
            let (mut open, mut fx_changed) = (true, false);
            egui::Window::new("Video adjustments").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                let Some(v) = self.video.clone() else { return };
                for (i, (name, prop)) in FX.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_sized([80.0, 18.0], egui::Label::new(*name));
                        if let Some(f) = Self::bar(ui, 180.0, (self.fx[i] as f32 + 100.0) / 200.0, 0.0, 5.0, true) {
                            self.fx[i] = (f * 200.0 - 100.0).round() as i64;
                            v.set_i64(prop, self.fx[i]);
                            fx_changed = true;
                        }
                        ui.label(RichText::new(format!("{:+}", self.fx[i])).monospace());
                    });
                }
                ui.separator();
                let mut vf = false;
                vf |= ui.checkbox(&mut self.sharpen, "Sharpen").changed();
                vf |= ui.checkbox(&mut self.denoise, "Denoise").changed();
                if vf {
                    self.apply_vf();
                    fx_changed = true;
                }
                if ui.button("Reset").clicked() {
                    for (i, (_, prop)) in FX.iter().enumerate() {
                        self.fx[i] = 0;
                        v.set_i64(prop, 0);
                    }
                    self.sharpen = false;
                    self.denoise = false;
                    self.apply_vf();
                    fx_changed = true;
                }
            });
            if fx_changed {
                self.save_settings();
            }
            self.fx_open = open;
        }

        if self.sub_open {
            let (mut open, mut changed) = (true, false);
            egui::Window::new("Subtitle style and languages").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.label(RichText::new("Appearance").strong());
                egui::Grid::new("sub_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label("Size");
                    changed |= ui.add(egui::Slider::new(&mut self.sub_scale, 0.5..=3.0).step_by(0.05).fixed_decimals(2)).changed();
                    ui.end_row();
                    ui.label("Outline");
                    changed |= ui.add(egui::Slider::new(&mut self.sub_border, 0.0..=8.0).step_by(0.5)).changed();
                    ui.end_row();
                    ui.label("Raise from bottom");
                    changed |= ui.add(egui::Slider::new(&mut self.sub_lift, 0..=40).suffix("%")).changed();
                    ui.end_row();
                    ui.label("Colour");
                    ui.horizontal(|ui| {
                        for (i, (name, _)) in SUB_COLORS.iter().enumerate() {
                            if ui.selectable_label(self.sub_color == i, *name).clicked() {
                                self.sub_color = i;
                                changed = true;
                            }
                        }
                    });
                    ui.end_row();
                });
                changed |= ui.checkbox(&mut self.sub_bold, "Bold").changed();
                changed |= ui.checkbox(&mut self.sub_ass, "Apply this style to styled (ASS/SSA) subtitles too").changed();
                if ui.button("Reset appearance").clicked() {
                    (self.sub_scale, self.sub_color, self.sub_border, self.sub_bold, self.sub_lift, self.sub_ass) = (1.0, 0, 3.0, false, 0, false);
                    changed = true;
                }
                ui.separator();
                ui.label(RichText::new("Languages and loading").strong());
                ui.label(RichText::new("Applies when the next file opens.").size(12.0).color(MUTED));
                egui::Grid::new("sub_lang_grid").num_columns(2).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label("Load subtitle files");
                    egui::ComboBox::from_id_salt("sub_auto").selected_text(SUB_AUTO[self.sub_auto].0).show_ui(ui, |ui| {
                        for (i, (name, _)) in SUB_AUTO.iter().enumerate() {
                            changed |= ui.selectable_value(&mut self.sub_auto, i, *name).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("Subtitle languages");
                    changed |= ui.add(egui::TextEdit::singleline(&mut self.slang).desired_width(200.0).hint_text("id,ind,en,eng")).changed();
                    ui.end_row();
                    ui.label("Audio languages");
                    changed |= ui.add(egui::TextEdit::singleline(&mut self.alang).desired_width(200.0).hint_text("ja,jpn,en,eng")).changed();
                    ui.end_row();
                });
                ui.label(RichText::new("Comma separated, in order of preference. Use both 2- and 3-letter codes: files tag languages either way.").size(12.0).color(MUTED));
            });
            if changed {
                self.apply_sub();
                self.save_settings();
            }
            self.sub_open = open;
        }

        if self.os_open {
            let mut open = true;
            let (mut search, mut pick) = (false, None);
            let busy = self.os_busy();
            egui::Window::new("Download subtitles").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).default_width(520.0).show(ctx, |ui| {
                egui::CollapsingHeader::new("OpenSubtitles account").default_open(self.os_creds.key.is_empty()).show(ui, |ui| {
                    egui::Grid::new("os_grid").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                        ui.label("API key");
                        ui.add(egui::TextEdit::singleline(&mut self.os_creds.key).password(true).desired_width(260.0));
                        ui.end_row();
                        ui.label("Username");
                        ui.add(egui::TextEdit::singleline(&mut self.os_creds.user).desired_width(260.0));
                        ui.end_row();
                        ui.label("Password");
                        ui.add(egui::TextEdit::singleline(&mut self.os_creds.pass).password(true).desired_width(260.0));
                        ui.end_row();
                    });
                    ui.label(RichText::new("Get a free API key at opensubtitles.com (Consumers section). Searching needs only the key; downloading also needs your login. Saved as plain text in the data folder.").size(12.0).color(MUTED));
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Languages");
                    ui.add(egui::TextEdit::singleline(&mut self.os_lang).desired_width(90.0).hint_text("en,id"));
                    let can = self.s.loaded && !busy;
                    if ui.add_enabled(can, egui::Button::new("Search")).clicked() {
                        search = true;
                    }
                    if busy {
                        ui.spinner();
                    }
                });
                if !self.s.loaded {
                    ui.label(RichText::new("Open a video first.").color(MUTED));
                }
                if !self.os_status.is_empty() {
                    ui.label(RichText::new(&self.os_status).size(12.0).color(MUTED));
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for h in &self.os_hits {
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!busy, egui::Button::new("Get")).clicked() {
                                pick = Some(h.clone());
                            }
                            ui.label(RichText::new(&h.lang).monospace().color(ACCENT));
                            if h.by_hash {
                                ui.label(RichText::new("exact").size(11.0).color(ACCENT));
                            }
                            ui.add(egui::Label::new(&h.name).truncate());
                            ui.label(RichText::new(format!("↓{}", h.downloads)).size(11.0).color(MUTED));
                        });
                    }
                });
            });
            if search {
                self.os_search();
            }
            if let Some(h) = pick {
                self.os_download(&h);
            }
            if !open {
                self.save_os_creds();
                self.save_settings();
            }
            self.os_open = open;
        }

        if self.upd_open {
            let mut open = true;
            let (mut install, mut skip) = (false, false);
            let (installing, checking) = (self.upd_installing, self.upd_rx.is_some() && !self.upd_installing);
            egui::Window::new("Software update").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.set_width(420.0);
                if let Some(rel) = &self.upd_rel {
                    ui.label(RichText::new(format!("{APP_NAME} {} is available", rel.version)).size(16.0).strong());
                    ui.label(RichText::new(format!("You have {VERSION}.")).color(MUTED));
                    if !rel.notes.trim().is_empty() {
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(170.0).show(ui, |ui| release_notes(ui, &rel.notes));
                    }
                    ui.add_space(8.0);
                    if installing {
                        let total = rel.asset_size.max(1) as f32;
                        let done = self.upd_progress.load(Ordering::Relaxed) as f32;
                        ui.add(egui::ProgressBar::new((done / total).min(1.0)).text(format!("{:.0} / {:.0} MB", done / 1_048_576.0, total / 1_048_576.0)));
                        ui.label(RichText::new("Layar restarts by itself when it is done.").size(12.0).color(MUTED));
                    } else {
                        ui.horizontal(|ui| {
                            if ui.button("Update and restart").clicked() { install = true; }
                            if ui.button("Later").clicked() { self.upd_open = false; }
                            if ui.button("Skip this version").clicked() { skip = true; }
                            ui.hyperlink_to("Release page", &rel.page);
                        });
                    }
                }
                if !self.upd_status.is_empty() && !installing {
                    ui.add_space(4.0);
                    ui.label(&self.upd_status);
                }
                if checking {
                    ui.spinner();
                }
            });
            if install {
                self.install_update();
            }
            if skip {
                if let Some(r) = &self.upd_rel {
                    self.upd_skip = r.version.clone();
                }
                self.save_settings();
                self.upd_open = false;
            }
            if !open && !self.upd_installing {
                self.upd_open = false;
            }
        }

        if self.help_open {
            let mut open = true;
            egui::Window::new("Keyboard shortcuts").pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                egui::Grid::new("help_grid").num_columns(2).spacing([24.0, 6.0]).show(ui, |ui| {
                    for (k, what) in SHORTCUTS {
                        ui.label(RichText::new(k).monospace().color(ACCENT));
                        ui.label(what);
                        ui.end_row();
                    }
                });
            });
            self.help_open = open;
        }

        if self.about_open {
            let mut open = true;
            let (mpv_v, ffmpeg_v) = self.video.as_ref().map_or_else(Default::default, |v| (v.str("mpv-version"), v.str("ffmpeg-version")));
            egui::Window::new(format!("About {APP_NAME}")).pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center()).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new(APP_NAME).size(22.0).strong());
                ui.label(RichText::new(format!("Version {VERSION}")).monospace().color(ACCENT));
                ui.add_space(6.0);
                ui.label("A video player written in Rust. egui draws the interface, libmpv decodes and plays.");
                ui.add_space(8.0);
                ui.separator();
                for (k, val) in [("Player core", mpv_v.as_str()), ("FFmpeg", ffmpeg_v.as_str()), ("UI", concat!("egui / eframe 0.37")), ("Fonts", "Inter, JetBrains Mono (SIL OFL 1.1)")] {
                    if !val.is_empty() {
                        ui.horizontal(|ui| {
                            ui.add_sized([90.0, 18.0], egui::Label::new(RichText::new(k).color(MUTED)));
                            ui.label(val);
                        });
                    }
                }
                ui.add_space(6.0);
                ui.label(RichText::new("mpv and FFmpeg are free software (GPL / LGPL). See the LICENSES folder next to the program.").size(12.0).color(MUTED));
                ui.add_space(4.0);
                ui.label(RichText::new(format!("Data folder: {}", data_dir().display())).size(12.0).color(MUTED));
            });
            self.about_open = open;
        }

        if self.url_open {
            let mut open = true;
            egui::Window::new("Open network stream").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.url).desired_width(360.0).hint_text("https://… or rtsp://…"));
                if self.url_focus {
                    r.request_focus();
                    self.url_focus = false;
                }
                let go = ui.button("Play").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
                if go && !self.url.trim().is_empty() {
                    let u = self.url.trim().to_string();
                    self.open(&[u], true);
                    self.url_open = false;
                }
            });
            if !open {
                self.url_open = false;
            }
        }
    }
}

fn main() -> eframe::Result {
    migrate_legacy_data();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title(APP_NAME).with_inner_size([1180.0, 720.0]).with_min_inner_size([640.0, 400.0]).with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
