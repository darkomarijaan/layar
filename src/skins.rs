//! Skins. Each one is a complete window layout with its own buttons, sliders and palette.
//! Graphite (the original layout) stays in main.rs; the other five live here.
//!
//! Every layout draws the same video (`stage`) and drives the same mpv state, so switching skin
//! never touches playback.

use super::*;
use egui::{Align, Align2, Area, FontId, Frame, Id, Layout, Margin, Order, Popup, Pos2, Rect, Response, Shape, StrokeKind, Ui, UiBuilder};

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Skin {
    #[default]
    Graphite,
    Cinema,
    Deck,
    Studio,
    Lounge,
    Terminal,
}

#[derive(Clone, Copy)]
pub struct Pal {
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub line: Color32,
    pub fg: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub light: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

impl Skin {
    pub const ALL: [Skin; 6] = [Skin::Graphite, Skin::Cinema, Skin::Deck, Skin::Studio, Skin::Lounge, Skin::Terminal];

    pub fn name(self) -> &'static str {
        match self {
            Skin::Graphite => "Graphite",
            Skin::Cinema => "Cinema",
            Skin::Deck => "Deck",
            Skin::Studio => "Studio",
            Skin::Lounge => "Lounge",
            Skin::Terminal => "Terminal",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Skin::Graphite => "graphite",
            Skin::Cinema => "cinema",
            Skin::Deck => "deck",
            Skin::Studio => "studio",
            Skin::Lounge => "lounge",
            Skin::Terminal => "terminal",
        }
    }

    pub fn from_key(k: &str) -> Option<Skin> {
        Skin::ALL.into_iter().find(|s| s.key() == k.trim())
    }

    pub fn pal(self) -> Pal {
        let p = |bg, surface, raised, line, fg, muted, accent, on_accent, light| Pal { bg, surface, raised, line, fg, muted, accent, on_accent, light };
        match self {
            Skin::Graphite => p(BG, SURFACE, RAISED, LINE, FG, MUTED, ACCENT, Color32::WHITE, false),
            Skin::Cinema => p(rgb(0, 0, 0), rgb(18, 24, 30), rgb(42, 50, 58), rgb(60, 70, 80), rgb(0xf3, 0xf7, 0xfa), rgb(0x98, 0xa4, 0xad), rgb(0x7b, 0xe0, 0xff), rgb(0x0a, 0x0f, 0x14), false),
            Skin::Deck => p(rgb(0x2b, 0x2e, 0x34), rgb(0x35, 0x39, 0x40), rgb(0x4a, 0x4f, 0x57), rgb(0x0d, 0x0e, 0x10), rgb(0xc9, 0xce, 0xd6), rgb(0x8b, 0x92, 0x9d), rgb(0x9d, 0xff, 0x7a), rgb(0x08, 0x12, 0x0a), false),
            Skin::Studio => p(rgb(0xee, 0xf0, 0xf4), rgb(0xff, 0xff, 0xff), rgb(0xe3, 0xe7, 0xee), rgb(0xd3, 0xd8, 0xdf), rgb(0x14, 0x17, 0x1c), rgb(0x5a, 0x62, 0x70), rgb(0x2f, 0x5b, 0xff), Color32::WHITE, true),
            Skin::Lounge => p(rgb(0x16, 0x11, 0x1f), rgb(0x1f, 0x18, 0x2c), rgb(0x2b, 0x22, 0x40), rgb(0x38, 0x2d, 0x50), Color32::WHITE, rgb(0xb3, 0xa9, 0xc7), rgb(0xff, 0x3d, 0x81), Color32::WHITE, false),
            Skin::Terminal => p(rgb(0x0b, 0x0a, 0x07), rgb(0x14, 0x11, 0x0b), rgb(0x1f, 0x1a, 0x10), rgb(0x7a, 0x5a, 0x22), rgb(0xff, 0xb3, 0x47), rgb(0xa6, 0x7a, 0x30), rgb(0xff, 0xb3, 0x47), rgb(0x0b, 0x0a, 0x07), false),
        }
    }

    pub fn visuals(self) -> egui::Visuals {
        if self == Skin::Graphite {
            return graphite_visuals();
        }
        let p = self.pal();
        let mut v = if p.light { egui::Visuals::light() } else { egui::Visuals::dark() };
        v.panel_fill = p.bg;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.raised;
        v.faint_bg_color = p.surface;
        v.override_text_color = Some(p.fg);
        v.selection.bg_fill = alpha(p.accent, 90);
        v.selection.stroke = Stroke::new(1.0, p.accent);
        v.hyperlink_color = p.accent;
        v.window_stroke = Stroke::new(1.0, p.line);
        let r = egui::CornerRadius::same(match self {
            Skin::Terminal => 0,
            Skin::Deck => 2,
            Skin::Studio => 3,
            Skin::Lounge => 12,
            _ => 8,
        });
        v.window_corner_radius = r;
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
            w.corner_radius = r;
        }
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
        v.widgets.inactive.bg_fill = p.raised;
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.hovered.bg_fill = p.raised;
        v.widgets.hovered.weak_bg_fill = p.raised;
        v.widgets.active.bg_fill = p.accent;
        v.widgets.active.weak_bg_fill = p.raised;
        v
    }
}

// ───────────────────────── drawing primitives ─────────────────────────

#[derive(Clone, Copy)]
pub enum G {
    Prev, Next, Play, Pause, Stop, Step, Full, Shot, Vol, Mute, List, Cc, Dots, Eject, Back, Fwd, Audio, Info, Fx, Folder,
}

/// Paint a glyph centred on `m`. `size` is the button edge it should fit in.
pub fn glyph(p: &egui::Painter, g: G, m: Pos2, size: f32, c: Color32) {
    let u = size * 0.27;
    let pt = |x: f32, y: f32| Pos2::new(m.x + x * u, m.y + y * u);
    let tri = |dir: f32, x: f32| p.add(Shape::convex_polygon(vec![pt(x - dir * 0.8, -1.0), pt(x - dir * 0.8, 1.0), pt(x + dir * 0.9, 0.0)], c, Stroke::NONE));
    let bar = |x: f32| p.rect_filled(Rect::from_center_size(pt(x, 0.0), Vec2::new(u * 0.35, u * 2.0)), 1.0, c);
    let st = Stroke::new((size / 18.0).max(1.4), c);
    match g {
        G::Prev => { bar(-1.0); tri(-1.0, 0.5); }
        G::Next => { tri(1.0, -0.5); bar(1.0); }
        G::Play => { tri(1.0, 0.1); }
        G::Pause => { bar(-0.6); bar(0.6); }
        G::Stop => { p.rect_filled(Rect::from_center_size(m, Vec2::splat(u * 1.8)), 2.0, c); }
        G::Step => {
            p.rect_stroke(Rect::from_center_size(pt(-0.3, 0.0), Vec2::new(u * 2.0, u * 1.6)), 2.0, st, StrokeKind::Inside);
            p.add(Shape::convex_polygon(vec![pt(-0.8, -0.5), pt(-0.8, 0.5), pt(0.1, 0.0)], c, Stroke::NONE));
            bar(1.7);
        }
        G::Full => {
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                p.line_segment([pt(sx * 1.2, sy * 1.2), pt(sx * 1.2, sy * 0.4)], st);
                p.line_segment([pt(sx * 1.2, sy * 1.2), pt(sx * 0.4, sy * 1.2)], st);
            }
        }
        G::Shot => {
            p.rect_stroke(Rect::from_center_size(pt(0.0, 0.2), Vec2::new(u * 2.6, u * 1.9)), 3.0, st, StrokeKind::Inside);
            p.rect_filled(Rect::from_center_size(pt(0.0, -0.95), Vec2::new(u * 1.0, u * 0.4)), 1.0, c);
            p.circle_stroke(pt(0.0, 0.2), u * 0.55, st);
        }
        G::Vol | G::Mute => {
            p.add(Shape::convex_polygon(vec![pt(-1.4, -0.5), pt(-0.7, -0.5), pt(0.1, -1.1), pt(0.1, 1.1), pt(-0.7, 0.5), pt(-1.4, 0.5)], c, Stroke::NONE));
            if matches!(g, G::Vol) {
                p.line_segment([pt(0.6, -0.5), pt(0.9, 0.0)], st);
                p.line_segment([pt(0.9, 0.0), pt(0.6, 0.5)], st);
                p.line_segment([pt(1.1, -1.0), pt(1.6, 0.0)], st);
                p.line_segment([pt(1.6, 0.0), pt(1.1, 1.0)], st);
            } else {
                p.line_segment([pt(0.6, -0.6), pt(1.6, 0.6)], st);
                p.line_segment([pt(0.6, 0.6), pt(1.6, -0.6)], st);
            }
        }
        G::List => {
            for (y, x1) in [(-0.9, 1.4), (0.0, 1.4), (0.9, 0.4)] {
                p.line_segment([pt(-1.4, y), pt(x1, y)], st);
            }
        }
        G::Cc => {
            p.rect_stroke(Rect::from_center_size(m, Vec2::new(u * 3.0, u * 2.2)), 3.0, st, StrokeKind::Inside);
            p.text(m, Align2::CENTER_CENTER, "CC", FontId::proportional(u * 0.95), c);
        }
        G::Dots => {
            for x in [-1.0, 0.0, 1.0] {
                p.circle_filled(pt(x, 0.0), u * 0.24, c);
            }
        }
        G::Eject => {
            p.add(Shape::convex_polygon(vec![pt(0.0, -1.2), pt(-1.3, 0.2), pt(1.3, 0.2)], c, Stroke::NONE));
            p.rect_filled(Rect::from_center_size(pt(0.0, 0.95), Vec2::new(u * 2.6, u * 0.4)), 1.0, c);
        }
        G::Back | G::Fwd => {
            let sx = if matches!(g, G::Fwd) { -1.0 } else { 1.0 };
            let r = 1.4;
            let pts: Vec<Pos2> = (0..=24)
                .map(|k| {
                    let a = (25.0 + k as f32 * 12.5_f32).to_radians();
                    pt(sx * a.sin() * r, -a.cos() * r)
                })
                .collect();
            p.add(Shape::line(pts, st));
            let a = 25.0_f32.to_radians();
            let (px, py) = (sx * a.sin() * r, -a.cos() * r);
            let t = (-a.cos() * sx, -a.sin());
            let n = (sx * a.sin(), -a.cos());
            p.add(Shape::convex_polygon(vec![pt(px + t.0 * 0.9, py + t.1 * 0.9), pt(px + n.0 * 0.5, py + n.1 * 0.5), pt(px - n.0 * 0.5, py - n.1 * 0.5)], c, Stroke::NONE));
            p.text(m + Vec2::new(0.0, u * 0.1), Align2::CENTER_CENTER, "10", FontId::proportional(u * 0.95), c);
        }
        G::Audio => {
            let pts: Vec<Pos2> = (0..=16)
                .map(|k| {
                    let a = k as f32 / 16.0 * std::f32::consts::PI;
                    pt(-a.cos() * 1.3, -a.sin() * 1.3 + 0.3)
                })
                .collect();
            p.add(Shape::line(pts, st));
            for x in [-1.3, 1.3] {
                p.rect_filled(Rect::from_center_size(pt(x, 0.75), Vec2::new(u * 0.7, u * 1.2)), 2.0, c);
            }
        }
        G::Info => {
            p.circle_stroke(m, u * 1.5, st);
            p.text(m, Align2::CENTER_CENTER, "i", FontId::proportional(u * 1.8), c);
        }
        G::Fx => {
            for (x, ky) in [(-1.1, 0.5), (0.0, -0.5), (1.1, 0.6)] {
                p.line_segment([pt(x, -1.3), pt(x, 1.3)], st);
                p.circle_filled(pt(x, ky), u * 0.38, c);
            }
        }
        G::Folder => {
            p.rect_filled(Rect::from_center_size(pt(0.0, 0.2), Vec2::new(u * 2.8, u * 1.8)), 2.0, c);
            p.rect_filled(Rect::from_min_size(pt(-1.4, -0.95), Vec2::new(u * 1.1, u * 0.4)), 1.0, c);
        }
    }
}

/// Icon button: optional filled background (`round` is the corner radius, `size / 2` makes a circle).
pub fn glyph_btn(ui: &mut Ui, g: G, size: f32, tip: &str, fill: Color32, hover: Color32, col: Color32, round: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let bg = if resp.hovered() { hover } else { fill };
    if bg != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, round, bg);
    }
    glyph(ui.painter(), g, rect.center(), size, col);
    resp.on_hover_text(tip)
}

pub fn gradient_v(p: &egui::Painter, r: Rect, top: Color32, bottom: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bottom);
    m.colored_vertex(r.left_bottom(), bottom);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    p.add(Shape::mesh(m));
}

pub struct BarLook {
    pub track: Color32,
    pub cache: Color32,
    pub fill: Color32,
    pub knob: Color32,
    pub thick: f32,
    pub round: f32,
    /// > 0: draw the fill as separate blocks this wide (LED look).
    pub seg: f32,
    pub rect_knob: bool,
}

/// Horizontal slider. Returns the new fraction (0..=1) while it is clicked or dragged.
pub fn hbar(ui: &mut Ui, width: f32, height: f32, frac: f32, cached: f32, k: &BarLook) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(10.0), height), Sense::click_and_drag());
    let track = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), k.thick));
    let hot = resp.hovered() || resp.dragged();
    let p = ui.painter();
    p.rect_filled(track, k.round, k.track);
    let x = |f: f32| track.left() + track.width() * f;
    if k.seg > 0.0 {
        let (mut sx, end) = (track.left() + 2.0, x(frac));
        while sx + k.seg <= end {
            p.rect_filled(Rect::from_min_size(Pos2::new(sx, track.top() + 2.0), Vec2::new(k.seg, track.height() - 4.0)), 0.0, k.fill);
            sx += k.seg * 1.4;
        }
    } else {
        if cached > 0.0 {
            p.rect_filled(Rect::from_min_max(track.min, Pos2::new(x(cached), track.bottom())), k.round, k.cache);
        }
        p.rect_filled(Rect::from_min_max(track.min, Pos2::new(x(frac), track.bottom())), k.round, k.fill);
    }
    if k.knob != Color32::TRANSPARENT {
        let c = Pos2::new(x(frac), track.center().y);
        if k.rect_knob {
            let r = Rect::from_center_size(c, Vec2::new(11.0, track.height() + 8.0));
            p.rect_filled(r, 1.5, k.knob);
            p.rect_stroke(r, 1.5, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
        } else {
            p.circle_filled(c, if hot { 7.0 } else { 5.5 }, k.knob);
        }
    }
    if resp.clicked() || resp.dragged() {
        return resp.interact_pointer_pos().map(|pt| ((pt.x - track.left()) / track.width()).clamp(0.0, 1.0));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_keys_round_trip() {
        for s in Skin::ALL {
            assert_eq!(Skin::from_key(s.key()), Some(s));
        }
        assert_eq!(Skin::from_key(" deck \n"), Some(Skin::Deck));
        assert_eq!(Skin::from_key("nope"), None);
        assert_eq!(Skin::default(), Skin::Graphite);
    }

    #[test]
    fn every_skin_has_readable_text() {
        // Text on the main background and on the accent must differ enough to read.
        let lum = |c: Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
        for s in Skin::ALL {
            let p = s.pal();
            assert!((lum(p.fg) - lum(p.bg)).abs() > 90.0, "{} fg/bg", s.name());
            assert!((lum(p.on_accent) - lum(p.accent)).abs() > 90.0, "{} on_accent/accent", s.name());
        }
    }

    #[test]
    fn hue_is_stable_and_in_range() {
        assert_eq!(name_hue("a.mkv"), name_hue("a.mkv"));
        assert!((0.0..1.0).contains(&name_hue("Edensor.2013.mp4")));
    }
}

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let h = (h.fract() + 1.0).fract() * 6.0;
    let (i, f) = (h.floor(), h - h.floor());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Stable colour for a file name, used for the Lounge cards.
fn name_hue(s: &str) -> f32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    (h % 360) as f32 / 360.0
}

fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Text button with a border; `arrow` adds a small drop-down triangle.
fn text_btn(ui: &mut Ui, p: &Pal, label: &str, arrow: bool, filled: bool, size: f32, round: f32) -> Response {
    let font = if ui.style().text_styles.get(&egui::TextStyle::Body).is_some_and(|f| f.family == egui::FontFamily::Monospace) { mono(size) } else { FontId::proportional(size) };
    let col = if filled { p.on_accent } else { p.fg };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, col);
    let w = galley.size().x + 22.0 + if arrow { 14.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, size + 14.0), Sense::click());
    let fill = if filled { p.accent } else if resp.hovered() { p.raised } else { p.surface };
    ui.painter().rect_filled(rect, round, fill);
    if !filled {
        ui.painter().rect_stroke(rect, round, Stroke::new(1.0, p.line), StrokeKind::Inside);
    }
    ui.painter().galley(Pos2::new(rect.left() + 11.0, rect.center().y - galley.size().y / 2.0), galley, col);
    if arrow {
        let c = Pos2::new(rect.right() - 12.0, rect.center().y);
        ui.painter().add(Shape::convex_polygon(vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(4.0, -2.0), c + Vec2::new(0.0, 3.0)], col, Stroke::NONE));
    }
    resp
}

impl App {
    fn frac(&self) -> f32 {
        if self.s.dur > 0.0 { (self.s.pos / self.s.dur).clamp(0.0, 1.0) as f32 } else { 0.0 }
    }

    fn cache_frac(&self) -> f32 {
        if self.s.dur > 0.0 { ((self.s.pos + self.s.cache) / self.s.dur).clamp(0.0, 1.0) as f32 } else { 0.0 }
    }

    fn seek_frac(&mut self, f: f32) {
        if self.s.dur <= 0.0 {
            return;
        }
        if let Some(v) = &self.video {
            v.cmd("seek", &[&(f as f64 * self.s.dur).to_string(), "absolute"]);
            self.seeking = true;
        }
    }

    fn set_volume_frac(&self, f: f32) {
        if let Some(v) = &self.video {
            v.set_f64("volume", (f * 200.0).round() as f64);
        }
    }

    fn sub_label(&self) -> String {
        self.s.subs.iter().find(|t| t.selected).map_or("off".to_string(), |t| t.id.to_string())
    }

    fn audio_label(&self) -> String {
        self.s.audio.iter().find(|t| t.selected).map_or("-".to_string(), |t| t.id.to_string())
    }

    /// Switch egui's look when the skin changes.
    pub(crate) fn apply_skin(&mut self, ctx: &egui::Context) {
        if self.skin_applied == Some(self.skin) {
            return;
        }
        let mut st = egui::Style::default();
        st.visuals = self.skin.visuals();
        if self.skin == Skin::Terminal {
            for f in st.text_styles.values_mut() {
                f.family = egui::FontFamily::Monospace;
            }
        }
        // Both themes get the skin, so an OS light/dark switch cannot swap it out.
        ctx.set_style_of(egui::Theme::Dark, st.clone());
        ctx.set_style_of(egui::Theme::Light, st);
        self.show_playlist = self.skin != Skin::Cinema; // Cinema keeps the picture clean until asked
        self.skin_applied = Some(self.skin);
        self.sub_pos_applied = f64::NAN;
    }

    pub(crate) fn layout_skin(&mut self, skin: Skin, ui: &mut Ui, ctx: &egui::Context) {
        match skin {
            Skin::Graphite => {}
            Skin::Cinema => self.layout_cinema(ui, ctx),
            Skin::Deck => self.layout_deck(ui, ctx),
            Skin::Studio => self.layout_studio(ui, ctx),
            Skin::Lounge => self.layout_lounge(ui, ctx),
            Skin::Terminal => self.layout_terminal(ui, ctx),
        }
    }

    /// Paint the video into `rect` and handle click (pause) and double-click (fullscreen).
    fn stage(&mut self, ui: &mut Ui, ctx: &egui::Context, rect: Rect) {
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        if let Some(v) = &self.video {
            v.paint(ui, rect);
        }
        if !self.s.loaded {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, "Drop a video here or press Ctrl+O", FontId::proportional(18.0), MUTED);
        }
        let resp = ui.interact(rect, Id::new("stage"), Sense::click());
        if resp.double_clicked() {
            let on = !self.fullscreen;
            self.set_fullscreen(ctx, on);
        } else if resp.clicked() {
            self.toggle_pause();
        }
    }

    // ── shared menus ──

    fn menu_popup(&mut self, ctx: &egui::Context, resp: &Response) {
        Popup::menu(resp).show(|ui| self.menu_items(ctx, ui));
    }

    fn sub_popup(&mut self, resp: &Response) {
        let Some(v) = self.video.clone() else { return };
        Popup::menu(resp).show(|ui| {
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
            if ui.button("Style and languages…").clicked() { self.sub_open = true; ui.close(); }
            if ui.button("Download subtitles…").clicked() { self.os_open = true; ui.close(); }
        });
    }

    fn audio_popup(&mut self, resp: &Response) {
        let Some(v) = self.video.clone() else { return };
        Popup::menu(resp).show(|ui| {
            if self.s.audio.is_empty() { ui.label("No audio tracks"); }
            for t in &self.s.audio {
                if ui.selectable_label(t.selected, &t.label).clicked() { v.set_i64("aid", t.id); ui.close(); }
            }
            ui.separator();
            if ui.button("Equalizer…").clicked() { self.eq_open = true; ui.close(); }
        });
    }

    fn speed_popup(&mut self, resp: &Response) {
        let Some(v) = self.video.clone() else { return };
        Popup::menu(resp).show(|ui| {
            for &sp in SPEEDS {
                if ui.selectable_label((self.s.speed - sp).abs() < 0.01, format!("{sp}×")).clicked() { v.set_f64("speed", sp); ui.close(); }
            }
        });
    }

    // ── side list (playlist / chapters / info) for skins that use a standard panel ──

    fn srow(ui: &mut Ui, p: &Pal, num: usize, text: &str, right: &str, now: bool, round: f32) -> bool {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
        if now {
            ui.painter().rect_filled(rect, round, alpha(p.accent, if p.light { 36 } else { 46 }));
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, round, p.raised);
        }
        let pr = ui.painter();
        pr.text(rect.left_center() + Vec2::new(12.0, 0.0), Align2::CENTER_CENTER, num.to_string(), mono(12.0), if now { p.accent } else { p.muted });
        let right_w = if right.is_empty() { 0.0 } else { 64.0 };
        pr.text(rect.right_center() + Vec2::new(-8.0, 0.0), Align2::RIGHT_CENTER, right, mono(12.0), p.muted);
        let clip = ui.painter().with_clip_rect(Rect::from_min_max(rect.min, Pos2::new(rect.right() - right_w, rect.bottom())));
        clip.text(rect.left_center() + Vec2::new(28.0, 0.0), Align2::LEFT_CENTER, text, FontId::proportional(13.0), p.fg);
        resp.clicked()
    }

    fn side_content(&mut self, ui: &mut Ui, p: &Pal, round: f32) {
        let Some(v) = self.video.clone() else { return };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = ui.available_width() / 3.0;
            for (t, name) in [(Tab::Playlist, "Playlist"), (Tab::Chapters, "Chapters"), (Tab::Info, "Info")] {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 32.0), Sense::click());
                let on = self.tab == t;
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, name, FontId::proportional(13.0), if on { p.fg } else { p.muted });
                if on {
                    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 2.0), rect.right_bottom()), 0.0, p.accent);
                }
                if resp.clicked() {
                    self.tab = t;
                }
            }
        });
        ui.separator();
        match self.tab {
            Tab::Playlist => {
                ui.horizontal(|ui| {
                    if ui.small_button("Add…").clicked() { self.pick_files(false); }
                    if ui.small_button("Clear").clicked() { v.cmd("stop", &[]); }
                });
                let mut jump = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.s.playlist.is_empty() {
                        ui.label(RichText::new("Drop files here or press Ctrl+O").color(p.muted));
                    }
                    for (i, name) in self.s.playlist.iter().enumerate() {
                        if Self::srow(ui, p, i + 1, name, "", i as i64 == self.s.plpos, round) { jump = Some(i); }
                    }
                });
                if let Some(i) = jump { v.set_i64("playlist-pos", i as i64); }
            }
            Tab::Chapters => {
                let mut jump = None;
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.s.chapters.is_empty() {
                        ui.label(RichText::new("No chapters in this file").color(p.muted));
                    }
                    for (i, (name, t)) in self.s.chapters.iter().enumerate() {
                        if Self::srow(ui, p, i + 1, name, &fmt_time(*t + 0.05), i as i64 == self.s.chapter, round) { jump = Some(i); }
                    }
                });
                if let Some(i) = jump { v.set_i64("chapter", i as i64); }
            }
            Tab::Info => {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if self.s.info.is_empty() {
                        ui.label(RichText::new("Nothing playing").color(p.muted));
                    }
                    for (k, val) in &self.s.info {
                        ui.label(RichText::new(k).size(11.0).color(p.muted));
                        ui.add(egui::Label::new(RichText::new(val).size(13.0)).wrap());
                        ui.add_space(6.0);
                    }
                });
            }
        }
    }

    // ───────────────────────────── Cinema ─────────────────────────────

    fn layout_cinema(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = Skin::Cinema.pal();
        let mut rect = Rect::NOTHING;
        egui::CentralPanel::default().frame(Frame::new().fill(Color32::BLACK)).show(ui, |ui| {
            rect = ui.max_rect();
            self.stage(ui, ctx, rect);
        });
        let visible = self.last_move.elapsed() < Duration::from_secs(3) || self.s.paused || !self.s.loaded;
        self.sub_base = if visible { 80.0 } else { 100.0 };
        if !visible {
            return;
        }
        let glass = Color32::from_white_alpha(30);
        let glass_hot = Color32::from_white_alpha(56);
        let Some(v) = self.video.clone() else { return };

        // Top: title and the two small buttons.
        Area::new(Id::new("cin_top")).order(Order::Middle).fixed_pos(rect.left_top()).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(rect.width(), 88.0), Sense::hover());
            gradient_v(ui.painter(), r, Color32::from_black_alpha(190), Color32::from_black_alpha(0));
            ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(Vec2::new(24.0, 16.0))).layout(Layout::left_to_right(Align::Min)), |ui| {
                ui.vertical(|ui| {
                    if self.s.loaded {
                        ui.label(RichText::new(&self.s.title).size(17.0).strong().color(p.fg));
                        ui.label(RichText::new(&self.s.video_info).size(12.0).color(p.muted));
                    } else {
                        ui.label(RichText::new(APP_NAME).size(17.0).strong().color(p.fg));
                    }
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    let r1 = glyph_btn(ui, G::Dots, 38.0, "Menu", glass, glass_hot, p.fg, 19.0);
                    self.menu_popup(ctx, &r1);
                    if glyph_btn(ui, G::List, 38.0, "Playlist (P)", glass, glass_hot, p.fg, 19.0).clicked() {
                        self.show_playlist = !self.show_playlist;
                    }
                });
            });
        });

        // Playlist drawer.
        if self.show_playlist {
            Area::new(Id::new("cin_list")).order(Order::Middle).fixed_pos(Pos2::new(rect.right() - 16.0, rect.top() + 64.0)).pivot(Align2::RIGHT_TOP).show(ctx, |ui| {
                Frame::new().fill(alpha(p.surface, 235)).stroke(Stroke::new(1.0, p.line)).corner_radius(16.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(300.0);
                    ui.set_height((rect.height() - 230.0).max(160.0));
                    self.side_content(ui, &p, 10.0);
                });
            });
        }

        // Bottom: glass dock.
        let w = (rect.width() * 0.76).clamp(440.0, 900.0).min(rect.width() - 24.0);
        Area::new(Id::new("cin_dock")).order(Order::Middle).fixed_pos(Pos2::new(rect.center().x, rect.bottom() - 20.0)).pivot(Align2::CENTER_BOTTOM).show(ctx, |ui| {
            Frame::new().fill(Color32::from_rgba_unmultiplied(14, 20, 26, 222)).stroke(Stroke::new(1.0, Color32::from_white_alpha(36))).corner_radius(30.0).inner_margin(Margin::symmetric(24, 14)).show(ui, |ui| {
                ui.set_width(w - 48.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(fmt_time(self.s.pos)).monospace().size(12.0).color(p.fg));
                    let bw = ui.available_width() - 74.0;
                    let look = BarLook { track: Color32::from_white_alpha(46), cache: Color32::from_white_alpha(70), fill: p.accent, knob: Color32::WHITE, thick: 4.0, round: 2.0, seg: 0.0, rect_knob: false };
                    if let Some(f) = hbar(ui, bw, 18.0, self.frac(), self.cache_frac(), &look) { self.seek_frac(f); }
                    ui.label(RichText::new(format!("-{}", fmt_time((self.s.dur - self.s.pos).max(0.0)))).monospace().size(12.0).color(p.muted));
                });
                let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::hover());
                // Left: volume.
                ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)), |ui| {
                    if glyph_btn(ui, if self.s.muted { G::Mute } else { G::Vol }, 34.0, "Mute (M)", Color32::TRANSPARENT, glass, p.fg, 17.0).clicked() { v.cmd("cycle", &["mute"]); }
                    let look = BarLook { track: Color32::from_white_alpha(46), cache: Color32::TRANSPARENT, fill: Color32::WHITE, knob: Color32::TRANSPARENT, thick: 4.0, round: 2.0, seg: 0.0, rect_knob: false };
                    if let Some(f) = hbar(ui, 90.0, 18.0, (self.s.volume / 200.0).clamp(0.0, 1.0) as f32, 0.0, &look) { self.set_volume_frac(f); }
                });
                // Centre: transport.
                let centre = Rect::from_center_size(row.center(), Vec2::new(280.0, 56.0));
                ui.scope_builder(UiBuilder::new().max_rect(centre).layout(Layout::left_to_right(Align::Center)), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if glyph_btn(ui, G::Prev, 32.0, "Previous (B)", Color32::TRANSPARENT, glass, p.fg, 16.0).clicked() { v.cmd("playlist-prev", &[]); }
                    if glyph_btn(ui, G::Back, 38.0, "Back 10 s", glass, glass_hot, p.fg, 19.0).clicked() { self.seek_rel(-10.0); }
                    if glyph_btn(ui, if self.s.paused { G::Play } else { G::Pause }, 52.0, "Play / pause (Space)", Color32::WHITE, Color32::from_gray(225), rgb(0x0a, 0x0f, 0x14), 26.0).clicked() { self.toggle_pause(); }
                    if glyph_btn(ui, G::Fwd, 38.0, "Forward 10 s", glass, glass_hot, p.fg, 19.0).clicked() { self.seek_rel(10.0); }
                    if glyph_btn(ui, G::Next, 32.0, "Next (N)", Color32::TRANSPARENT, glass, p.fg, 16.0).clicked() { v.cmd("playlist-next", &[]); }
                });
                // Right: subtitles, audio, fullscreen.
                ui.scope_builder(UiBuilder::new().max_rect(row).layout(Layout::right_to_left(Align::Center)), |ui| {
                    if glyph_btn(ui, G::Full, 34.0, "Fullscreen (F)", Color32::TRANSPARENT, glass, p.fg, 17.0).clicked() {
                        let on = !self.fullscreen;
                        self.set_fullscreen(ctx, on);
                    }
                    let a = glyph_btn(ui, G::Audio, 34.0, "Audio track", Color32::TRANSPARENT, glass, p.fg, 17.0);
                    self.audio_popup(&a);
                    let s = glyph_btn(ui, G::Cc, 34.0, "Subtitles", Color32::TRANSPARENT, glass, p.fg, 17.0);
                    self.sub_popup(&s);
                });
            });
        });
    }

    // ───────────────────────────── Deck ─────────────────────────────

    fn bevel_btn(ui: &mut Ui, p: &Pal, g: G, lit: bool, tip: &str) -> Response {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(54.0, 34.0), Sense::click());
        let pr = ui.painter();
        let down = lit || resp.is_pointer_button_down_on();
        let face = Rect::from_min_size(rect.min + Vec2::new(0.0, if down { 3.0 } else { 0.0 }), Vec2::new(rect.width(), rect.height() - 3.0));
        pr.rect_filled(Rect::from_min_max(rect.min + Vec2::new(0.0, 3.0), rect.max), 2.0, p.line); // the key's lower edge
        pr.rect_filled(face, 2.0, if down { rgb(0x2f, 0x33, 0x38) } else if resp.hovered() { rgb(0x5b, 0x61, 0x6b) } else { rgb(0x4f, 0x55, 0x5e) });
        pr.line_segment([face.left_top() + Vec2::new(2.0, 1.0), face.right_top() + Vec2::new(-2.0, 1.0)], Stroke::new(1.0, rgb(0x86, 0x8e, 0x99)));
        pr.rect_stroke(face, 2.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
        glyph(pr, g, face.center(), 28.0, if lit { p.accent } else { p.fg });
        resp.on_hover_text(tip)
    }

    fn led_toggle(ui: &mut Ui, p: &Pal, label: &str, on: bool) -> Response {
        let galley = ui.painter().layout_no_wrap(label.to_string(), mono(12.0), p.fg);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 24.0, 30.0), Sense::click());
        let c = Pos2::new(rect.left() + 7.0, rect.center().y);
        if on {
            ui.painter().circle_filled(c, 7.0, alpha(p.accent, 60));
        }
        ui.painter().circle_filled(c, 4.0, if on { p.accent } else { rgb(0x1a, 0x2a, 0x1d) });
        ui.painter().circle_stroke(c, 4.0, Stroke::new(1.0, Color32::BLACK));
        ui.painter().galley(Pos2::new(rect.left() + 18.0, rect.center().y - galley.size().y / 2.0), galley, p.fg);
        resp
    }

    fn layout_deck(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = Skin::Deck.pal();
        let lcd_bg = rgb(8, 18, 10);
        let lcd = p.accent;
        let Some(v) = self.video.clone() else { return };
        self.sub_base = 100.0;

        egui::Panel::top("deck_top").exact_size(30.0).frame(Frame::new().fill(rgb(0x3b, 0x3f, 0x46)).stroke(Stroke::new(1.0, p.line)).inner_margin(Margin::symmetric(10, 3))).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("LAYAR  DECK").monospace().size(13.0).color(p.fg));
                ui.add_space(14.0);
                let r = text_btn(ui, &p, "MENU", true, false, 12.0, 2.0);
                self.menu_popup(ctx, &r);
            });
        });

        egui::Panel::bottom("deck_ctl").exact_size(106.0).frame(Frame::new().fill(p.bg).stroke(Stroke::new(1.0, p.line)).inner_margin(Margin::symmetric(12, 10))).show(ui, |ui| {
            let w = ui.available_width();
            let look = BarLook { track: rgb(7, 8, 10), cache: Color32::TRANSPARENT, fill: lcd, knob: rgb(0x8b, 0x92, 0x9d), thick: 16.0, round: 1.0, seg: 5.0, rect_knob: true };
            if let Some(f) = hbar(ui, w, 24.0, self.frac(), 0.0, &look) { self.seek_frac(f); }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let on = self.s.loaded;
                if Self::bevel_btn(ui, &p, G::Prev, false, "Previous (B)").clicked() { v.cmd("playlist-prev", &[]); }
                if Self::bevel_btn(ui, &p, G::Play, on && !self.s.paused, "Play").clicked() && self.s.paused { self.toggle_pause(); }
                if Self::bevel_btn(ui, &p, G::Pause, on && self.s.paused, "Pause").clicked() && !self.s.paused { self.toggle_pause(); }
                if Self::bevel_btn(ui, &p, G::Stop, false, "Stop").clicked() { v.cmd("stop", &[]); }
                if Self::bevel_btn(ui, &p, G::Next, false, "Next (N)").clicked() { v.cmd("playlist-next", &[]); }
                if Self::bevel_btn(ui, &p, G::Step, false, "Next frame (.)").clicked() { v.cmd("frame-step", &[]); }
                if Self::bevel_btn(ui, &p, G::Eject, false, "Open files (Ctrl+O)").clicked() { self.pick_files(true); }
                ui.add_space(14.0);
                // The lamps are shortcuts to things the menu also offers: drop them when the window is narrow.
                if ui.available_width() > 400.0 {
                    if Self::led_toggle(ui, &p, "EQ", self.eq_open).clicked() { self.eq_open = !self.eq_open; }
                    if Self::led_toggle(ui, &p, "PL", self.show_playlist).clicked() { self.show_playlist = !self.show_playlist; }
                    if Self::led_toggle(ui, &p, "SUB", self.sub_open).clicked() { self.sub_open = !self.sub_open; }
                    if Self::led_toggle(ui, &p, "A-B", self.s.ab_loop).clicked() { v.cmd("ab-loop", &[]); }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let look = BarLook { track: rgb(7, 8, 10), cache: Color32::TRANSPARENT, fill: lcd, knob: Color32::TRANSPARENT, thick: 14.0, round: 1.0, seg: 4.0, rect_knob: false };
                    if let Some(f) = hbar(ui, 120.0, 22.0, (self.s.volume / 200.0).clamp(0.0, 1.0) as f32, 0.0, &look) { self.set_volume_frac(f); }
                    if glyph_btn(ui, if self.s.muted { G::Mute } else { G::Vol }, 30.0, "Mute (M)", Color32::TRANSPARENT, p.raised, p.fg, 2.0).clicked() { v.cmd("cycle", &["mute"]); }
                });
            });
        });

        let lcd_frame = || Frame::new().fill(lcd_bg).stroke(Stroke::new(2.0, rgb(3, 6, 3))).inner_margin(Margin::symmetric(8, 6));
        egui::Panel::right("deck_side").exact_size(296.0).resizable(false).frame(Frame::new().fill(p.bg).stroke(Stroke::new(1.0, p.line)).inner_margin(Margin::same(10))).show(ui, |ui| {
            lcd_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(fmt_time(self.s.pos)).monospace().size(38.0).color(lcd));
                ui.add(egui::Label::new(RichText::new(if self.s.loaded { self.s.title.to_uppercase() } else { "NO FILE".into() }).monospace().size(11.0).color(lcd)).truncate());
            });
            ui.add_space(8.0);
            lcd_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                let line = |ui: &mut Ui, k: &str, val: &str| {
                    ui.add(egui::Label::new(RichText::new(format!("{k:<7}{val}")).monospace().size(11.0).color(lcd)).truncate());
                };
                line(ui, "VIDEO", if self.s.video_info.is_empty() { "-" } else { &self.s.video_info });
                line(ui, "DECODE", if self.s.hwdec.is_empty() { "-" } else { &self.s.hwdec });
                line(ui, "SPEED", &format!("{}x", self.s.speed));
                line(ui, "AUDIO", &self.audio_label());
                line(ui, "SUB", &self.sub_label());
            });
            ui.add_space(8.0);
            if self.show_playlist {
                let h = ui.available_height();
                let mut jump = None;
                lcd_frame().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(h - 14.0);
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        if self.s.playlist.is_empty() {
                            ui.label(RichText::new("DROP FILES HERE").monospace().size(12.0).color(lcd));
                        }
                        for (i, name) in self.s.playlist.iter().enumerate() {
                            let now = i as i64 == self.s.plpos;
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                            if now { ui.painter().rect_filled(rect, 0.0, lcd); }
                            let clip = ui.painter().with_clip_rect(rect);
                            clip.text(rect.left_center() + Vec2::new(2.0, 0.0), Align2::LEFT_CENTER, format!("{}. {}", i + 1, name), mono(12.0), if now { lcd_bg } else { lcd });
                            if resp.clicked() { jump = Some(i); }
                        }
                    });
                });
                if let Some(i) = jump { v.set_i64("playlist-pos", i as i64); }
            }
        });

        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(Margin::same(10))).show(ui, |ui| {
            let outer = ui.max_rect();
            let pr = ui.painter();
            // Bevelled screen frame: dark top-left, light bottom-right.
            pr.rect_filled(outer, 0.0, Color32::BLACK);
            let (dark, light) = (Stroke::new(3.0, rgb(0x0a, 0x0b, 0x0d)), Stroke::new(3.0, rgb(0x55, 0x5b, 0x64)));
            pr.line_segment([outer.left_top(), outer.right_top()], dark);
            pr.line_segment([outer.left_top(), outer.left_bottom()], dark);
            pr.line_segment([outer.left_bottom(), outer.right_bottom()], light);
            pr.line_segment([outer.right_top(), outer.right_bottom()], light);
            self.stage(ui, ctx, outer.shrink(4.0));
        });
    }

    // ───────────────────────────── Studio ─────────────────────────────

    /// Rectangular button with an optional icon and a label.
    fn label_btn(ui: &mut Ui, p: &Pal, g: Option<G>, label: &str, primary: bool, icon_after: bool) -> Response {
        let col = if primary { p.on_accent } else { p.fg };
        let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(13.0), col);
        let icon_w = if g.is_some() { 22.0 } else { 0.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 28.0 + icon_w, 34.0), Sense::click());
        let fill = if primary { if resp.hovered() { alpha(p.accent, 225) } else { p.accent } } else if resp.hovered() { p.raised } else { p.surface };
        ui.painter().rect_filled(rect, 3.0, fill);
        if !primary {
            ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, rgb(0xc5, 0xcb, 0xd5)), StrokeKind::Inside);
        }
        let mut x = rect.left() + 14.0;
        let mid = rect.center().y;
        if let (Some(g), false) = (g, icon_after) {
            glyph(ui.painter(), g, Pos2::new(x + 7.0, mid), 22.0, col);
            x += icon_w;
        }
        ui.painter().galley(Pos2::new(x, mid - galley.size().y / 2.0), galley.clone(), col);
        if let (Some(g), true) = (g, icon_after) {
            glyph(ui.painter(), g, Pos2::new(x + galley.size().x + 15.0, mid), 22.0, col);
        }
        resp
    }

    fn layout_studio(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = Skin::Studio.pal();
        let Some(v) = self.video.clone() else { return };
        self.sub_base = 100.0;
        let rail_line = rgb(0xd3, 0xd8, 0xdf);

        egui::Panel::left("studio_rail").exact_size(96.0).resizable(false).frame(Frame::new().fill(p.surface).stroke(Stroke::new(1.0, rail_line))).show(ui, |ui| {
            ui.add_space(14.0);
            ui.vertical_centered(|ui| ui.label(RichText::new(APP_NAME).size(18.0).strong().color(p.fg)));
            ui.add_space(14.0);
            let items: [(G, &str, bool); 6] = [
                (G::Folder, "Library", false),
                (G::List, "Queue", self.show_playlist && self.tab == Tab::Playlist),
                (G::Cc, "Subtitles", self.sub_open),
                (G::Audio, "Audio", self.eq_open),
                (G::Fx, "Effects", self.fx_open),
                (G::Info, "Info", self.show_playlist && self.tab == Tab::Info),
            ];
            for (i, (g, label, on)) in items.into_iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 66.0), Sense::click());
                let col = if on { p.accent } else { p.muted };
                if on {
                    ui.painter().rect_filled(rect, 0.0, rgb(0xea, 0xf0, 0xff));
                    ui.painter().rect_filled(Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())), 0.0, p.accent);
                } else if resp.hovered() {
                    ui.painter().rect_filled(rect, 0.0, p.bg);
                }
                glyph(ui.painter(), g, rect.center() - Vec2::new(0.0, 9.0), 26.0, col);
                ui.painter().text(rect.center() + Vec2::new(0.0, 18.0), Align2::CENTER_CENTER, label, FontId::proportional(11.0), col);
                if resp.clicked() {
                    match i {
                        0 => self.pick_files(true),
                        1 => { if self.show_playlist && self.tab == Tab::Playlist { self.show_playlist = false } else { self.tab = Tab::Playlist; self.show_playlist = true } }
                        2 => self.sub_open = !self.sub_open,
                        3 => self.eq_open = !self.eq_open,
                        4 => self.fx_open = !self.fx_open,
                        _ => { if self.show_playlist && self.tab == Tab::Info { self.show_playlist = false } else { self.tab = Tab::Info; self.show_playlist = true } }
                    }
                }
            }
        });

        egui::Panel::top("studio_top").exact_size(46.0).frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(14, 6))).show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let path = self.s.path.replace('\\', "/");
                let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                if self.s.loaded && parts.len() > 1 {
                    let from = parts.len().saturating_sub(3);
                    ui.label(RichText::new(format!("{} /", parts[from..parts.len() - 1].join(" / "))).color(p.muted));
                }
                ui.label(RichText::new(if self.s.loaded { self.s.title.as_str() } else { "No file open" }).strong().color(p.fg));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let m = text_btn(ui, &p, "Menu", true, false, 13.0, 3.0);
                    self.menu_popup(ctx, &m);
                    let sp = text_btn(ui, &p, &format!("{}×", self.s.speed), false, false, 13.0, 3.0);
                    self.speed_popup(&sp);
                    let au = text_btn(ui, &p, &format!("Audio: {}", self.audio_label()), true, false, 13.0, 3.0);
                    self.audio_popup(&au);
                    let sb = text_btn(ui, &p, &format!("Subtitle: {}", self.sub_label()), true, false, 13.0, 3.0);
                    self.sub_popup(&sb);
                });
            });
        });

        egui::Panel::bottom("studio_bottom").exact_size(112.0).frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(14, 8))).show(ui, |ui| {
            // Timeline: one block per chapter, filled up to the playhead.
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click_and_drag());
            let dur = self.s.dur.max(0.001);
            let frac = self.frac();
            let mut spans: Vec<(String, f32, f32)> = Vec::new();
            if self.s.chapters.is_empty() {
                spans.push((String::new(), 0.0, 1.0));
            } else {
                for (i, (name, t)) in self.s.chapters.iter().enumerate() {
                    let end = self.s.chapters.get(i + 1).map_or(1.0, |c| (c.1 / dur) as f32);
                    spans.push((name.clone(), (*t / dur) as f32, end));
                }
            }
            let band = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + 10.0), Pos2::new(rect.right(), rect.bottom() - 2.0));
            let x = |f: f32| band.left() + band.width() * f;
            for (name, a, b) in &spans {
                let seg = Rect::from_min_max(Pos2::new(x(*a) + 1.0, band.top()), Pos2::new(x(*b) - 1.0, band.bottom()));
                ui.painter().rect_filled(seg, 2.0, p.raised);
                let done = frac.clamp(*a, *b);
                if done > *a {
                    ui.painter().rect_filled(Rect::from_min_max(seg.min, Pos2::new(x(done) - 1.0, seg.bottom())), 2.0, p.accent);
                }
                if !name.is_empty() {
                    ui.painter().with_clip_rect(seg).text(seg.left_center() + Vec2::new(6.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(11.0), if done >= *b { p.on_accent } else { p.muted });
                }
            }
            if self.s.loaded {
                let px = x(frac);
                ui.painter().line_segment([Pos2::new(px, rect.top() + 4.0), Pos2::new(px, band.bottom() + 2.0)], Stroke::new(2.0, p.fg));
                ui.painter().add(Shape::convex_polygon(vec![Pos2::new(px - 5.0, rect.top()), Pos2::new(px + 5.0, rect.top()), Pos2::new(px, rect.top() + 7.0)], p.fg, Stroke::NONE));
            }
            if resp.clicked() || resp.dragged() {
                if let Some(pt) = resp.interact_pointer_pos() { self.seek_frac(((pt.x - band.left()) / band.width()).clamp(0.0, 1.0)); }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if Self::label_btn(ui, &p, Some(G::Prev), "Prev", false, false).clicked() { v.cmd("playlist-prev", &[]); }
                let (g, label) = if self.s.paused { (G::Play, "Play") } else { (G::Pause, "Pause") };
                if Self::label_btn(ui, &p, Some(g), label, true, false).clicked() { self.toggle_pause(); }
                if Self::label_btn(ui, &p, Some(G::Next), "Next", false, true).clicked() { v.cmd("playlist-next", &[]); }
                ui.add_space(10.0);
                // Narrow windows: drop the duration, then the volume label, so nothing overlaps.
                let room = ui.available_width();
                if room > 330.0 {
                    ui.label(RichText::new(format!("{} / {}", fmt_time(self.s.pos), fmt_time(self.s.dur))).monospace().size(14.0).color(p.fg));
                } else if room > 250.0 {
                    ui.label(RichText::new(fmt_time(self.s.pos)).monospace().size(14.0).color(p.fg));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if glyph_btn(ui, G::Full, 34.0, "Fullscreen (F)", p.surface, p.raised, p.fg, 3.0).clicked() {
                        let on = !self.fullscreen;
                        self.set_fullscreen(ctx, on);
                    }
                    if glyph_btn(ui, G::Shot, 34.0, "Screenshot (S)", p.surface, p.raised, p.fg, 3.0).clicked() { v.cmd("screenshot", &[]); }
                    let vol = self.s.volume.round() as i64;
                    if Self::label_btn(ui, &p, None, "+", false, false).clicked() { v.set_f64("volume", (self.s.volume + 5.0).min(200.0)); }
                    ui.label(RichText::new(format!("{vol}")).monospace().size(13.0).color(p.fg));
                    if Self::label_btn(ui, &p, None, "−", false, false).clicked() { v.set_f64("volume", (self.s.volume - 5.0).max(0.0)); }
                    if room > 400.0 {
                        ui.label(RichText::new("VOL").size(11.0).color(p.muted));
                    }
                });
            });
        });

        if self.show_playlist {
            egui::Panel::right("studio_side").exact_size(290.0).resizable(false).frame(Frame::new().fill(p.surface).stroke(Stroke::new(1.0, rail_line)).inner_margin(Margin::same(8))).show(ui, |ui| {
                self.side_content(ui, &p, 3.0);
            });
        }

        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(14, 0))).show(ui, |ui| {
            let rect = ui.max_rect();
            self.stage(ui, ctx, rect);
        });
    }

    // ───────────────────────────── Lounge ─────────────────────────────

    fn pill(ui: &mut Ui, p: &Pal, g: Option<G>, label: &str, primary: bool) -> Response {
        let col = if primary { p.on_accent } else { p.fg };
        let galley = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(14.0), col);
        let icon_w = if g.is_some() { 24.0 } else { 0.0 };
        let h = if primary { 46.0 } else { 40.0 };
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x + 36.0 + icon_w + if primary { 8.0 } else { 0.0 }, h), Sense::click());
        let fill = if primary { if resp.hovered() { alpha(p.accent, 230) } else { p.accent } } else if resp.hovered() { Color32::from_white_alpha(52) } else { Color32::from_white_alpha(34) };
        ui.painter().rect_filled(rect, h / 2.0, fill);
        let mut x = rect.left() + 18.0 + if primary { 4.0 } else { 0.0 };
        if let Some(g) = g {
            glyph(ui.painter(), g, Pos2::new(x + 9.0, rect.center().y), 26.0, col);
            x += icon_w;
        }
        ui.painter().galley(Pos2::new(x, rect.center().y - galley.size().y / 2.0), galley, col);
        resp
    }

    fn layout_lounge(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = Skin::Lounge.pal();
        let Some(v) = self.video.clone() else { return };

        // Playlist as a strip of poster cards.
        egui::Panel::bottom("lounge_next").exact_size(176.0).frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 36, right: 36, top: 12, bottom: 12 })).show(ui, |ui| {
            ui.label(RichText::new("UP NEXT").size(11.0).color(p.muted).strong());
            ui.add_space(6.0);
            let mut jump = None;
            if self.s.playlist.is_empty() {
                ui.label(RichText::new("Drop files here or press Ctrl+O").color(p.muted));
            }
            egui::ScrollArea::horizontal().auto_shrink([false, false]).show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    for (i, name) in self.s.playlist.iter().enumerate() {
                        let now = i as i64 == self.s.plpos;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(196.0, 110.0), Sense::click());
                        let hue = name_hue(name);
                        let pr = ui.painter();
                        pr.rect_filled(rect, 14.0, hsv(hue, 0.55, if resp.hovered() { 0.85 } else { 0.72 }));
                        let low = Rect::from_min_max(Pos2::new(rect.left(), rect.top() + rect.height() * 0.55), rect.right_bottom());
                        pr.rect_filled(low, egui::CornerRadius { nw: 0, ne: 0, sw: 14, se: 14 }, Color32::from_black_alpha(95));
                        pr.text(Pos2::new(rect.center().x, rect.top() + rect.height() * 0.34), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(32.0), Color32::from_white_alpha(120));
                        pr.with_clip_rect(low.shrink2(Vec2::new(10.0, 0.0))).text(low.left_center() + Vec2::new(10.0, 0.0), Align2::LEFT_CENTER, name, FontId::proportional(12.0), Color32::WHITE);
                        if now {
                            pr.rect_stroke(rect, 14.0, Stroke::new(3.0, p.accent), StrokeKind::Outside);
                            pr.rect_filled(Rect::from_min_size(Pos2::new(rect.left() + 10.0, rect.bottom() - 6.0), Vec2::new((rect.width() - 20.0) * self.frac(), 3.0)), 1.5, p.accent);
                        }
                        if resp.clicked() { jump = Some(i); }
                    }
                });
            });
            if let Some(i) = jump { v.set_i64("playlist-pos", i as i64); }
        });

        let mut rect = Rect::NOTHING;
        egui::CentralPanel::default().frame(Frame::new().fill(Color32::BLACK)).show(ui, |ui| {
            rect = ui.max_rect();
            self.stage(ui, ctx, rect);
        });
        let visible = self.last_move.elapsed() < Duration::from_secs(3) || self.s.paused || !self.s.loaded;
        self.sub_base = if visible { 66.0 } else { 100.0 };
        if !visible {
            return;
        }

        let fade = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - 250.0_f32.min(rect.height())), rect.right_bottom());
        let mut open_menu = None;
        Area::new(Id::new("lounge_info")).order(Order::Middle).fixed_pos(fade.left_top()).show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(fade.size(), Sense::hover());
            gradient_v(ui.painter(), r, alpha(p.bg, 0), alpha(p.bg, 245));
            ui.scope_builder(UiBuilder::new().max_rect(r.shrink2(Vec2::new(36.0, 14.0))).layout(Layout::bottom_up(Align::Min)), |ui| {
                // Progress (bottom-most).
                ui.horizontal(|ui| {
                    let look = BarLook { track: Color32::from_white_alpha(50), cache: Color32::from_white_alpha(60), fill: p.accent, knob: Color32::TRANSPARENT, thick: 8.0, round: 4.0, seg: 0.0, rect_knob: false };
                    let bw = (ui.available_width() - 120.0).max(60.0);
                    if let Some(f) = hbar(ui, bw, 20.0, self.frac(), self.cache_frac(), &look) { self.seek_frac(f); }
                    let left = ((self.s.dur - self.s.pos).max(0.0) / 60.0).ceil() as i64;
                    ui.label(RichText::new(if self.s.loaded { format!("{left} min left") } else { String::new() }).strong().size(13.0).color(p.fg));
                });
                ui.add_space(18.0);
                // Pills.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if self.s.loaded {
                        let (g, label) = if self.s.paused { (G::Play, format!("Resume {}", fmt_time(self.s.pos))) } else { (G::Pause, "Pause".to_string()) };
                        if Self::pill(ui, &p, Some(g), &label, true).clicked() { self.toggle_pause(); }
                    } else if Self::pill(ui, &p, Some(G::Folder), "Open file", true).clicked() {
                        self.pick_files(true);
                    }
                    let s = Self::pill(ui, &p, Some(G::Cc), &format!("Subtitle {}", self.sub_label()), false);
                    self.sub_popup(&s);
                    let a = Self::pill(ui, &p, Some(G::Audio), &format!("Audio {}", self.audio_label()), false);
                    self.audio_popup(&a);
                    let sp = Self::pill(ui, &p, None, &format!("{}×", self.s.speed), false);
                    self.speed_popup(&sp);
                    if Self::pill(ui, &p, Some(G::Full), "Fullscreen", false).clicked() {
                        let on = !self.fullscreen;
                        self.set_fullscreen(ctx, on);
                    }
                    let m = Self::pill(ui, &p, Some(G::Dots), "Menu", false);
                    open_menu = Some(m);
                });
                ui.add_space(10.0);
                // Title and facts.
                ui.label(RichText::new(if self.s.loaded { self.s.title.as_str() } else { APP_NAME }).size(34.0).strong().color(p.fg));
                if self.s.loaded {
                    ui.horizontal(|ui| {
                        for chip in [self.s.video_info.clone(), if self.s.dur > 0.0 { fmt_time(self.s.dur) } else { String::new() }, self.s.hwdec.clone()].into_iter().filter(|c| !c.is_empty()) {
                            let galley = ui.painter().layout_no_wrap(chip, FontId::proportional(12.0), p.fg);
                            let (cr, _) = ui.allocate_exact_size(Vec2::new(galley.size().x + 18.0, 22.0), Sense::hover());
                            ui.painter().rect_filled(cr, 11.0, Color32::from_white_alpha(36));
                            ui.painter().galley(Pos2::new(cr.left() + 9.0, cr.center().y - galley.size().y / 2.0), galley, p.fg);
                        }
                    });
                }
            });
        });
        if let Some(m) = open_menu {
            self.menu_popup(ctx, &m);
        }
    }

    // ───────────────────────────── Terminal ─────────────────────────────

    fn tbox(ui: &Ui, p: &Pal, rect: Rect, title: &str) {
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let galley = ui.painter().layout_no_wrap(title.to_string(), mono(12.0), p.fg);
        let tr = Rect::from_min_size(Pos2::new(rect.left() + 12.0, rect.top() - galley.size().y / 2.0), galley.size()).expand2(Vec2::new(6.0, 0.0));
        ui.painter().rect_filled(tr, 0.0, p.bg);
        ui.painter().galley(tr.min + Vec2::new(6.0, 0.0), galley, p.fg);
    }

    fn keycap(ui: &mut Ui, p: &Pal, key: &str, what: &str) -> Response {
        let kg = ui.painter().layout_no_wrap(key.to_string(), mono(12.0), p.fg);
        let wg = ui.painter().layout_no_wrap(what.to_string(), mono(12.0), p.muted);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(kg.size().x + wg.size().x + 24.0, 22.0), Sense::click());
        let kr = Rect::from_min_size(rect.min, Vec2::new(kg.size().x + 12.0, rect.height()));
        if resp.hovered() {
            ui.painter().rect_filled(kr, 0.0, p.fg);
        }
        ui.painter().rect_stroke(kr, 0.0, Stroke::new(1.0, p.fg), StrokeKind::Inside);
        ui.painter().galley(kr.min + Vec2::new(6.0, (kr.height() - kg.size().y) / 2.0), kg, if resp.hovered() { p.bg } else { p.fg });
        ui.painter().galley(Pos2::new(kr.right() + 7.0, rect.center().y - wg.size().y / 2.0), wg, p.muted);
        resp
    }

    fn layout_terminal(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let p = Skin::Terminal.pal();
        let Some(v) = self.video.clone() else { return };
        self.sub_base = 100.0;

        egui::Panel::bottom("term_status").exact_size(24.0).frame(Frame::new().fill(p.fg).inner_margin(Margin::symmetric(10, 2))).show(ui, |ui| {
            let state = if !self.s.loaded { "IDLE" } else if self.s.paused { "PAUSED" } else { "PLAYING" };
            let left = format!("{state}  {}", if self.s.loaded { self.s.title.as_str() } else { "" });
            let right = format!("vol {:.0}%  {}x  sub:{}  aid:{}  {}", self.s.volume, self.s.speed, self.sub_label(), self.audio_label(), if self.s.hwdec.is_empty() { "-" } else { &self.s.hwdec });
            let r = ui.max_rect();
            let pr = ui.painter().with_clip_rect(r);
            pr.text(r.left_center(), Align2::LEFT_CENTER, left, mono(12.5), p.bg);
            pr.text(r.right_center(), Align2::RIGHT_CENTER, right, mono(12.5), p.bg);
        });

        egui::Panel::left("term_left").exact_size(300.0).resizable(false).frame(Frame::new().fill(p.bg).inner_margin(Margin::same(14))).show(ui, |ui| {
            let r = ui.max_rect();
            let split = r.top() + (r.height() * 0.58).round();
            let qbox = Rect::from_min_max(r.min + Vec2::new(0.0, 6.0), Pos2::new(r.right(), split - 8.0));
            let ibox = Rect::from_min_max(Pos2::new(r.left(), split + 8.0), r.right_bottom());
            Self::tbox(ui, &p, qbox, "QUEUE");
            Self::tbox(ui, &p, ibox, "INFO");
            let mut jump = None;
            ui.scope_builder(UiBuilder::new().max_rect(qbox.shrink2(Vec2::new(8.0, 12.0))), |ui| {
                egui::ScrollArea::vertical().id_salt("term_queue").auto_shrink([false, false]).show(ui, |ui| {
                    if self.s.playlist.is_empty() {
                        ui.label(RichText::new("drop files here").color(p.muted));
                    }
                    for (i, name) in self.s.playlist.iter().enumerate() {
                        let now = i as i64 == self.s.plpos;
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::click());
                        if now { ui.painter().rect_filled(rect, 0.0, p.fg); } else if resp.hovered() { ui.painter().rect_filled(rect, 0.0, p.raised); }
                        ui.painter().with_clip_rect(rect).text(rect.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, format!("{} {} {}", if now { ">" } else { " " }, i + 1, name), mono(12.5), if now { p.bg } else { p.fg });
                        if resp.clicked() { jump = Some(i); }
                    }
                });
            });
            if let Some(i) = jump { v.set_i64("playlist-pos", i as i64); }
            ui.scope_builder(UiBuilder::new().max_rect(ibox.shrink2(Vec2::new(10.0, 14.0))), |ui| {
                egui::ScrollArea::vertical().id_salt("term_info").auto_shrink([false, false]).show(ui, |ui| {
                    for (k, val) in self.s.info.iter().take(12) {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 17.0), Sense::hover());
                        let pr = ui.painter().with_clip_rect(rect);
                        pr.text(rect.left_center(), Align2::LEFT_CENTER, k.to_lowercase(), mono(11.5), p.muted);
                        pr.text(rect.right_center(), Align2::RIGHT_CENTER, val, mono(11.5), p.fg);
                    }
                });
            });
        });

        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(Margin { left: 4, right: 14, top: 20, bottom: 12 })).show(ui, |ui| {
            let r = ui.max_rect();
            let keys_h = if r.width() < 520.0 { 82.0 } else { 54.0 };
            let vbox = Rect::from_min_max(r.min, Pos2::new(r.right(), r.bottom() - keys_h - 34.0));
            Self::tbox(ui, &p, vbox, "NOW PLAYING");
            self.stage(ui, ctx, vbox.shrink(5.0));

            // Progress line: time, block bar, time, percent.
            let pr_rect = Rect::from_min_size(Pos2::new(r.left(), vbox.bottom() + 8.0), Vec2::new(r.width(), 22.0));
            let left = format!("{} ", fmt_time(self.s.pos));
            let right = format!(" {}  {:>3.0}%", fmt_time(self.s.dur), self.frac() * 100.0);
            let lg = ui.painter().layout_no_wrap(left, mono(13.0), p.fg);
            let rg = ui.painter().layout_no_wrap(right, mono(13.0), p.muted);
            let bar = Rect::from_min_max(Pos2::new(pr_rect.left() + lg.size().x + 4.0, pr_rect.top() + 3.0), Pos2::new(pr_rect.right() - rg.size().x - 4.0, pr_rect.bottom() - 3.0));
            let resp = ui.interact(bar, Id::new("term_seek"), Sense::click_and_drag());
            let (lx, rx) = (pr_rect.left(), pr_rect.right() - rg.size().x);
            let (lh, rh) = (lg.size().y, rg.size().y);
            ui.painter().galley(Pos2::new(lx, pr_rect.center().y - lh / 2.0), lg, p.fg);
            ui.painter().galley(Pos2::new(rx, pr_rect.center().y - rh / 2.0), rg, p.muted);
            let (bw, gap) = (6.0, 2.0);
            let n = ((bar.width() + gap) / (bw + gap)).floor().max(1.0) as usize;
            let on = (self.frac() * n as f32).round() as usize;
            for i in 0..n {
                let b = Rect::from_min_size(Pos2::new(bar.left() + i as f32 * (bw + gap), bar.top()), Vec2::new(bw, bar.height()));
                ui.painter().rect_filled(b, 0.0, if i < on { p.fg } else { p.line });
            }
            if resp.clicked() || resp.dragged() {
                if let Some(pt) = resp.interact_pointer_pos() { self.seek_frac(((pt.x - bar.left()) / bar.width()).clamp(0.0, 1.0)); }
            }

            // Key hints double as buttons.
            let keys_rect = Rect::from_min_size(Pos2::new(r.left(), pr_rect.bottom() + 8.0), Vec2::new(r.width(), keys_h));
            ui.scope_builder(UiBuilder::new().max_rect(keys_rect).layout(Layout::left_to_right(Align::Min).with_main_wrap(true)), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(16.0, 8.0);
                let paused = self.s.paused;
                // Menu first, so it stays reachable however narrow the window gets.
                let m = Self::keycap(ui, &p, "MENU", "all");
                self.menu_popup(ctx, &m);
                if Self::keycap(ui, &p, "SPACE", if paused { "play" } else { "pause" }).clicked() { self.toggle_pause(); }
                if Self::keycap(ui, &p, "B", "prev").clicked() { v.cmd("playlist-prev", &[]); }
                if Self::keycap(ui, &p, "N", "next").clicked() { v.cmd("playlist-next", &[]); }
                if Self::keycap(ui, &p, "←", "-5s").clicked() { self.seek_rel(-5.0); }
                if Self::keycap(ui, &p, "→", "+5s").clicked() { self.seek_rel(5.0); }
                if Self::keycap(ui, &p, "M", "mute").clicked() { v.cmd("cycle", &["mute"]); }
                if Self::keycap(ui, &p, "F", "full").clicked() { let on = !self.fullscreen; self.set_fullscreen(ctx, on); }
                let s = Self::keycap(ui, &p, "S", "subs");
                self.sub_popup(&s);
                let a = Self::keycap(ui, &p, "A", "audio");
                self.audio_popup(&a);
            });
        });
    }
}
