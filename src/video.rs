//! libmpv rendered into an offscreen GL texture, then drawn inside an egui paint callback.
use eframe::{
    egui, egui_glow,
    glow::{self, HasContext},
};
use libmpv2::{
    Mpv,
    render::{OpenGLInitParams, RenderContext, RenderParam, RenderParamApiType},
};
use std::{
    ffi::{CStr, CString, c_void},
    num::NonZeroU32,
    sync::{Arc, Mutex},
};

/// Where settings and resume positions live: `data/` next to the exe when `portable.txt` sits beside it,
/// otherwise `%APPDATA%\Layar`.
pub fn data_dir() -> std::path::PathBuf {
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        if dir.join("portable.txt").exists() {
            return dir.join("data");
        }
    }
    std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("Layar")
}

/// One-time copy of the old `vlc_rust` data folder (settings, resume positions) to `Layar`.
/// The old folder is left in place. Does nothing in portable mode or once `Layar` exists.
pub fn migrate_legacy_data() {
    let new = data_dir();
    let Some(parent) = new.parent() else { return };
    let old = parent.join("vlc_rust");
    if new.exists() || !old.is_dir() {
        return;
    }
    fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            let target = to.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                copy_dir(&entry.path(), &target)?;
            } else {
                std::fs::copy(entry.path(), target)?;
            }
        }
        Ok(())
    }
    let _ = copy_dir(&old, &new);
}

pub type GetProc = Arc<dyn Fn(&CStr) -> *const c_void + Send + Sync>;

fn get_proc(ctx: &GetProc, name: &str) -> *mut c_void {
    CString::new(name).map_or(std::ptr::null_mut(), |c| ctx(&c) as *mut c_void)
}

const VS: &str = "#version 330 core
out vec2 uv;
void main(){ vec2 p = vec2(float((gl_VertexID<<1)&2), float(gl_VertexID&2)); uv = p; gl_Position = vec4(p*2.0-1.0, 0.0, 1.0); }";
const FS: &str = "#version 330 core
uniform sampler2D t; in vec2 uv; out vec4 c;
void main(){ c = vec4(texture(t, uv).rgb, 1.0); }";

struct Gl {
    render: RenderContext<'static>,
    prog: glow::Program,
    vao: glow::VertexArray,
    fbo: Option<glow::Framebuffer>,
    tex: Option<glow::Texture>,
    size: (i32, i32),
}
// ponytail: only ever touched from the GL thread under a Mutex.
unsafe impl Send for Gl {}

impl Gl {
    unsafe fn ensure_target(&mut self, gl: &glow::Context, w: i32, h: i32) {
        unsafe {
            if self.fbo.is_some() && self.size == (w, h) {
                return;
            }
            if let Some(f) = self.fbo.take() {
                gl.delete_framebuffer(f);
            }
            if let Some(t) = self.tex.take() {
                gl.delete_texture(t);
            }
            let tex = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(tex));
            gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, w, h, 0, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None));
            for (p, v) in [
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
            ] {
                gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
            }
            let fbo = gl.create_framebuffer().unwrap();
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(tex), 0);
            self.fbo = Some(fbo);
            self.tex = Some(tex);
            self.size = (w, h);
        }
    }

    unsafe fn draw(&mut self, gl: &glow::Context, vp: egui::epaint::ViewportInPixels) {
        unsafe {
            let (w, h) = (vp.width_px.max(1), vp.height_px.max(1));
            let prev_fbo = gl.get_parameter_i32(glow::FRAMEBUFFER_BINDING);
            let mut view = [0i32; 4];
            gl.get_parameter_i32_slice(glow::VIEWPORT, &mut view);
            let mut scissor = [0i32; 4];
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor);
            let scissor_on = gl.is_enabled(glow::SCISSOR_TEST);

            self.ensure_target(gl, w, h);
            let fbo = self.fbo.unwrap();
            let _ = self.render.render::<()>(fbo.0.get() as i32, w, h, true);

            gl.bind_framebuffer(glow::FRAMEBUFFER, NonZeroU32::new(prev_fbo as u32).map(glow::NativeFramebuffer));
            gl.viewport(view[0], view[1], view[2], view[3]);
            gl.scissor(scissor[0], scissor[1], scissor[2], scissor[3]);
            if scissor_on {
                gl.enable(glow::SCISSOR_TEST)
            } else {
                gl.disable(glow::SCISSOR_TEST)
            }

            gl.disable(glow::BLEND);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            gl.use_program(Some(self.prog));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.tex);
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            gl.enable(glow::BLEND);
            gl.blend_equation_separate(glow::FUNC_ADD, glow::FUNC_ADD);
            gl.blend_func_separate(glow::ONE, glow::ONE_MINUS_SRC_ALPHA, glow::ONE_MINUS_DST_ALPHA, glow::ONE);
        }
    }
}

/// An mpv instance plus the GL state that shows its frames.
#[derive(Clone)]
pub struct Video {
    pub mpv: &'static Mpv,
    gl: Arc<Mutex<Gl>>,
}

impl Video {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let gl = cc.gl.clone().ok_or("no GL context")?;
        let proc_addr: GetProc = cc.get_proc_address.clone().ok_or("no get_proc_address")?;
        let watch_later = data_dir().join("watch_later").to_string_lossy().into_owned();
        let _ = std::fs::create_dir_all(&watch_later);
        let shots = std::env::var("USERPROFILE").map(|u| format!("{u}\\Pictures")).unwrap_or_default();

        let mpv = Mpv::with_initializer(|i| {
            i.set_property("vo", "libmpv")?;
            i.set_property("hwdec", "auto-safe")?;
            i.set_property("keep-open", "yes")?;
            i.set_property("hr-seek", "yes")?; // exact seeks, not keyframe-snapped
            // Layar draws its own UI, so mpv's built-in Lua scripts (OSC, stats, console) are dead weight: ~50 MB.
            for (k, v) in [("load-scripts", "no"), ("osc", "no"), ("load-stats-overlay", "no"), ("load-osd-console", "no"), ("ytdl", "no"), ("input-default-bindings", "no")] {
                let _ = i.set_property(k, v); // option names vary by mpv build; a missing one is harmless
            }
            i.set_property("volume-max", 200i64)?;
            i.set_property("save-position-on-quit", true)?;
            i.set_property("resume-playback", true)?;
            i.set_property("watch-later-directory", watch_later.as_str())?;
            i.set_property("screenshot-directory", shots.as_str())?;
            // Debug aid: LAYAR_IPC=<pipe name> lets an outside script read mpv properties.
            if let Ok(pipe) = std::env::var("LAYAR_IPC") {
                i.set_property("input-ipc-server", pipe.as_str())?;
            }
            Ok(())
        })
        .map_err(|e| format!("mpv init: {e}"))?;
        // ponytail: leaked on purpose, the player lives as long as the process.
        let mpv: &'static Mpv = Box::leak(Box::new(mpv));

        let mut render = mpv
            .create_render_context(vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams { get_proc_address: get_proc, ctx: proc_addr }),
            ])
            .map_err(|e| format!("mpv render: {e}"))?;
        let egui_ctx = cc.egui_ctx.clone();
        render.set_update_callback(move || egui_ctx.request_repaint());

        let (prog, vao) = unsafe {
            let prog = gl.create_program()?;
            for (kind, src) in [(glow::VERTEX_SHADER, VS), (glow::FRAGMENT_SHADER, FS)] {
                let s = gl.create_shader(kind)?;
                gl.shader_source(s, src);
                gl.compile_shader(s);
                if !gl.get_shader_compile_status(s) {
                    return Err(gl.get_shader_info_log(s));
                }
                gl.attach_shader(prog, s);
            }
            gl.link_program(prog);
            if !gl.get_program_link_status(prog) {
                return Err(gl.get_program_info_log(prog));
            }
            (prog, gl.create_vertex_array()?)
        };
        let gl = Arc::new(Mutex::new(Gl { render, prog, vao, fbo: None, tex: None, size: (0, 0) }));
        Ok(Self { mpv, gl })
    }

    /// Paint the current frame into `rect` (mpv letterboxes inside it).
    pub fn paint(&self, ui: &egui::Ui, rect: egui::Rect) {
        let gl = self.gl.clone();
        let cb = egui_glow::CallbackFn::new(move |info, painter| {
            if let Ok(mut g) = gl.lock() {
                unsafe { g.draw(painter.gl(), info.viewport_in_pixels()) };
            }
        });
        ui.painter().add(egui::PaintCallback { rect, callback: Arc::new(cb) });
    }

    // Thin mpv helpers. Errors are ignored on purpose: nothing loaded, bad track id, ...
    pub fn cmd(&self, name: &str, args: &[&str]) {
        let _ = self.mpv.command(name, args);
    }
    pub fn f64(&self, k: &str) -> f64 {
        self.mpv.get_property::<f64>(k).unwrap_or(0.0)
    }
    pub fn i64(&self, k: &str) -> i64 {
        self.mpv.get_property::<i64>(k).unwrap_or(0)
    }
    pub fn bool(&self, k: &str) -> bool {
        self.mpv.get_property::<bool>(k).unwrap_or(false)
    }
    pub fn str(&self, k: &str) -> String {
        self.mpv.get_property::<String>(k).unwrap_or_default()
    }
    pub fn set_f64(&self, k: &str, v: f64) {
        let _ = self.mpv.set_property(k, v);
    }
    pub fn set_bool(&self, k: &str, v: bool) {
        let _ = self.mpv.set_property(k, v);
    }
    pub fn set_str(&self, k: &str, v: &str) {
        let _ = self.mpv.set_property(k, v);
    }
    pub fn set_i64(&self, k: &str, v: i64) {
        let _ = self.mpv.set_property(k, v);
    }
}
