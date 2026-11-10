//! Self-update from GitHub Releases (blocking; call from a worker thread).
//!
//! Flow: ask the API for the latest release, compare versions, download the portable zip,
//! check its SHA-256 against the digest GitHub publishes for the asset, pull `Layar.exe` out,
//! and swap it in. Windows lets a running exe be renamed but not overwritten, so the old one
//! becomes `Layar.exe.old` (deleted on the next start).

use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub const REPO: &str = "darkomarijaan/layar";
const UA: &str = concat!("Layar/", env!("CARGO_PKG_VERSION"));
const MAX_ZIP: u64 = 400 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub sha256: Option<String>,
}

pub enum Msg {
    /// Result of a check: `Some` only when a newer release exists.
    Checked(Option<Release>),
    Installed,
    Error(String),
}

/// "v1.2.3" -> [1, 2, 3]; anything after a '-' or '+' (pre-release, build) is ignored.
fn parse(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (mut a, mut b) = (parse(candidate), parse(current));
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    a > b
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_connect(Some(Duration::from_secs(15))).timeout_recv_body(Some(Duration::from_secs(300))).build().into()
}

fn pick(v: &serde_json::Value) -> Result<Release, String> {
    let asset = v["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["name"].as_str().is_some_and(|n| n.ends_with("win64-portable.zip")))
        .ok_or("The latest release has no Windows zip")?;
    // Only download from GitHub itself.
    let asset_url = asset["browser_download_url"].as_str().filter(|u| u.starts_with("https://github.com/")).ok_or("Bad download link")?;
    Ok(Release {
        version: v["tag_name"].as_str().unwrap_or("").trim_start_matches(['v', 'V']).to_string(),
        notes: v["body"].as_str().unwrap_or("").to_string(),
        page: v["html_url"].as_str().unwrap_or("").to_string(),
        asset_url: asset_url.to_string(),
        asset_size: asset["size"].as_u64().unwrap_or(0),
        sha256: asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(|d| d.to_lowercase()),
    })
}

/// Look up the latest published release. `Ok(None)` means you are up to date.
pub fn check(repo: &str, current: &str) -> Result<Option<Release>, String> {
    let mut r = agent()
        .get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .header("User-Agent", UA)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::StatusCode(404) => "No release found".to_string(),
            ureq::Error::StatusCode(403 | 429) => "GitHub rate limit reached, try again later".to_string(),
            e => e.to_string(),
        })?;
    let v: serde_json::Value = serde_json::from_str(&r.body_mut().read_to_string().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let rel = pick(&v)?;
    Ok(is_newer(&rel.version, current).then_some(rel))
}

/// Download the release zip, verify it, and replace `exe` with the `Layar.exe` inside.
pub fn install(rel: &Release, exe: &Path, progress: &AtomicU64) -> Result<(), String> {
    let want = rel.sha256.as_deref().ok_or("This release has no published checksum, so it cannot be verified. Download it from the release page instead.")?;
    let tmp = std::env::temp_dir().join(format!("layar-update-{}.zip", rel.version));
    let result = (|| {
        let mut resp = agent().get(&rel.asset_url).header("User-Agent", UA).call().map_err(|e| e.to_string())?;
        let mut reader = resp.body_mut().with_config().limit(MAX_ZIP).reader();
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let (mut hasher, mut buf, mut total) = (Sha256::new(), vec![0u8; 64 * 1024], 0u64);
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            total += n as u64;
            progress.store(total, Ordering::Relaxed);
        }
        drop(file);
        let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != want {
            return Err("Checksum mismatch: the download is corrupt or was tampered with, nothing was changed".to_string());
        }
        let bytes = extract_exe(&tmp)?;
        swap(exe, &bytes)
    })();
    let _ = std::fs::remove_file(&tmp);
    result
}

fn extract_exe(zip_path: &Path) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().map_err(|e| e.to_string())?.replace('\\', "/");
        if name.rsplit('/').next() == Some("Layar.exe") {
            let mut out = Vec::with_capacity(f.size() as usize);
            f.take(MAX_ZIP).read_to_end(&mut out).map_err(|e| e.to_string())?;
            if out.len() < 1024 || &out[..2] != b"MZ" {
                return Err("The zip does not contain a valid Layar.exe".into());
            }
            return Ok(out);
        }
    }
    Err("The zip does not contain Layar.exe".into())
}

fn old_path(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Put `bytes` in place of `exe`, keeping the old file as `<exe>.old`; roll back on failure.
fn swap(exe: &Path, bytes: &[u8]) -> Result<(), String> {
    let (old, new) = (old_path(exe), {
        let mut s = exe.as_os_str().to_owned();
        s.push(".new");
        PathBuf::from(s)
    });
    let denied = |e: std::io::Error| format!("Cannot write to {}: {e}. Move Layar to a folder you can write to, or download the update manually.", exe.parent().map_or(String::new(), |p| p.display().to_string()));
    std::fs::write(&new, bytes).map_err(denied)?;
    let _ = std::fs::remove_file(&old);
    if let Err(e) = std::fs::rename(exe, &old) {
        let _ = std::fs::remove_file(&new);
        return Err(denied(e));
    }
    if let Err(e) = std::fs::rename(&new, exe) {
        let _ = std::fs::rename(&old, exe);
        let _ = std::fs::remove_file(&new);
        return Err(denied(e));
    }
    Ok(())
}

/// Remove the previous exe left behind by an update. Call once at startup.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
    }
}

/// Start the freshly installed exe. Command line arguments are not forwarded.
pub fn restart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.2.1"));
        assert!(!is_newer("0.2.0-beta", "0.2.0"));
        assert!(!is_newer("garbage", "0.2.0"));
    }

    #[test]
    fn swap_replaces_and_keeps_old() {
        let dir = std::env::temp_dir().join(format!("layar-swap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Layar.exe");
        std::fs::write(&exe, b"old").unwrap();
        swap(&exe, b"new").unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new");
        assert_eq!(std::fs::read(old_path(&exe)).unwrap(), b"old");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Network: the real release asset downloads, verifies and installs into a scratch dir.
    #[test]
    #[ignore]
    fn live_install_from_github() {
        let rel = check(REPO, "0.0.1").unwrap().expect("a release newer than 0.0.1");
        assert!(rel.sha256.is_some(), "release asset has no digest");
        let dir = std::env::temp_dir().join(format!("layar-live-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("Layar.exe");
        std::fs::write(&exe, b"stub").unwrap();
        let p = AtomicU64::new(0);
        install(&rel, &exe, &p).unwrap();
        let installed = std::fs::read(&exe).unwrap();
        assert_eq!(&installed[..2], b"MZ");
        assert!(p.load(Ordering::Relaxed) > 1_000_000);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
