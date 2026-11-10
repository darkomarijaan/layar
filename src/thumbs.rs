//! Video thumbnails, made with libmpv itself (no ffmpeg needed on the user's PC).
//!
//! A short-lived second mpv instance with `vo=image` decodes one frame at 12 % of the file,
//! scaled to 320 px wide, and writes a JPEG. Finished thumbnails are cached in the data folder,
//! keyed by path, size and modification time, so a file is only decoded once.
//! Call `make` from a worker thread.

use libmpv2::{Mpv, events::Event};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, UNIX_EPOCH},
};

pub struct Thumb {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

/// Cache key for a local file; `None` for anything that is not a readable file (URLs, missing files).
pub fn key(path: &str) -> Option<String> {
    let md = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
    let secs = md.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let mut h: u64 = 0xcbf29ce484222325; // FNV-1a
    for b in path.as_bytes().iter().copied().chain(md.len().to_le_bytes()).chain(secs.to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    Some(format!("{h:016x}"))
}

pub fn make(path: &str, dir: &Path) -> Result<Thumb, String> {
    let key = key(path).ok_or("not a local file")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let cached = dir.join(format!("{key}.jpg"));
    if !cached.is_file() {
        generate(path, &cached, dir, &key)?;
    }
    let img = image::open(&cached).map_err(|e| e.to_string())?.into_rgba8();
    Ok(Thumb { w: img.width() as usize, h: img.height() as usize, rgba: img.into_raw() })
}

fn generate(path: &str, out: &Path, dir: &Path, key: &str) -> Result<(), String> {
    let tmp: PathBuf = dir.join(format!("tmp-{}-{key}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = run_mpv(path, &tmp).and_then(|()| {
        let jpg = std::fs::read_dir(&tmp)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "jpg"))
            .ok_or("no video frame (audio only, or the file could not be decoded)")?;
        // Rename can fail across volumes; copying always works.
        std::fs::rename(&jpg, out).or_else(|_| std::fs::copy(&jpg, out).map(|_| ())).map_err(|e| e.to_string())
    });
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn run_mpv(path: &str, outdir: &Path) -> Result<(), String> {
    let outdir = outdir.to_string_lossy().into_owned();
    let mpv = Mpv::with_initializer(|i| {
        for (k, v) in [
            ("vo", "image"),
            ("vo-image-format", "jpg"),
            ("vo-image-jpeg-quality", "82"),
            ("vo-image-outdir", outdir.as_str()),
            ("vf", "lavfi=[scale=320:-2]"),
            ("frames", "1"),
            ("start", "12%"),
            ("ao", "null"),
            ("aid", "no"),
            ("sid", "no"),
            ("hwdec", "no"),
            ("audio-display", "embedded-first"), // cover art counts as the picture for music files
            ("load-scripts", "no"),
            ("osc", "no"),
            ("ytdl", "no"),
            ("input-default-bindings", "no"),
            ("terminal", "no"),
        ] {
            let _ = i.set_property(k, v); // an option a given mpv build lacks is harmless
        }
        Ok(())
    })
    .map_err(|e| format!("mpv init: {e}"))?;
    mpv.command("loadfile", &[path, "replace"]).map_err(|e| format!("loadfile: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if Instant::now() > deadline {
            return Err("timed out".into());
        }
        match mpv.wait_event(0.5) {
            Some(Ok(Event::EndFile(_))) | Some(Ok(Event::Shutdown)) => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_rejects_non_files_and_is_stable() {
        assert!(key("https://example.com/a.mkv").is_none());
        assert!(key("C:/definitely/not/here.mkv").is_none());
        let exe = std::env::current_exe().unwrap();
        let p = exe.to_string_lossy();
        assert_eq!(key(&p), key(&p));
        assert_eq!(key(&p).unwrap().len(), 16);
    }

    /// Needs a real video: LAYAR_TEST_VIDEO=<file> cargo test thumbs -- --ignored
    #[test]
    #[ignore]
    fn makes_a_thumbnail_from_a_real_video() {
        let video = std::env::var("LAYAR_TEST_VIDEO").expect("set LAYAR_TEST_VIDEO");
        let dir = std::env::temp_dir().join(format!("layar-thumbs-{}", std::process::id()));
        let t = make(&video, &dir).unwrap();
        assert!(t.w > 0 && t.w <= 320 && t.h > 0, "size {}x{}", t.w, t.h);
        assert_eq!(t.rgba.len(), t.w * t.h * 4);
        // Not a blank frame: the picture has variation.
        let first = &t.rgba[..4];
        assert!(t.rgba.chunks(4).any(|px| px[..3] != first[..3]), "frame is a single colour");
        // Second call is served from the cache.
        let again = make(&video, &dir).unwrap();
        assert_eq!((again.w, again.h), (t.w, t.h));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
