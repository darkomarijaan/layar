//! OpenSubtitles.com REST API client (blocking; call it from a worker thread).
//! Needs the user's own API key; downloading also needs their account login.

use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
};

const UA: &str = concat!("Layar v", env!("CARGO_PKG_VERSION"));
const HOST: &str = "api.opensubtitles.com";

#[derive(Clone)]
pub struct Creds {
    pub key: String,
    pub user: String,
    pub pass: String,
}

#[derive(Clone)]
pub struct Hit {
    pub name: String,
    pub lang: String,
    pub file_id: i64,
    pub downloads: i64,
    pub by_hash: bool,
}

pub enum Msg {
    Hits(Vec<Hit>),
    Saved(PathBuf),
    Error(String),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(25))).build().into()
}

fn err(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401) => "Unauthorized (check API key / login)".into(),
        ureq::Error::StatusCode(406) => "Daily download quota reached".into(),
        ureq::Error::StatusCode(429) => "Too many requests, try again in a moment".into(),
        other => other.to_string(),
    }
}

fn json_of(mut r: ureq::http::Response<ureq::Body>) -> Result<Value, String> {
    let text = r.body_mut().read_to_string().map_err(err)?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// OpenSubtitles file hash: file size plus the 64-bit words of the first and last 64 KiB.
pub fn movie_hash(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let size = f.metadata().ok()?.len();
    if size < 65536 {
        return None;
    }
    let mut h = size;
    let mut buf = [0u8; 65536];
    let mut add = |buf: &[u8; 65536]| {
        for c in buf.chunks_exact(8) {
            h = h.wrapping_add(u64::from_le_bytes(c.try_into().unwrap()));
        }
    };
    f.read_exact(&mut buf).ok()?;
    add(&buf);
    f.seek(SeekFrom::Start(size - 65536)).ok()?;
    f.read_exact(&mut buf).ok()?;
    add(&buf);
    Some(format!("{h:016x}"))
}

/// "My.Show.S01E02.1080p.mkv" -> "My Show S01E02 1080p"
pub fn title_query(title: &str) -> String {
    let stem = Path::new(title).file_stem().map_or(title.to_string(), |s| s.to_string_lossy().into_owned());
    stem.replace(['.', '_'], " ").trim().to_string()
}

fn search_once(a: &ureq::Agent, key: &str, lang: &str, hash: Option<&str>, query: &str) -> Result<Vec<Hit>, String> {
    let mut req = a.get(format!("https://{HOST}/api/v1/subtitles")).header("Api-Key", key).header("User-Agent", UA).header("Accept", "application/json");
    // Parameters must be sent in alphabetical order for the API's cache.
    if let Some(h) = hash {
        req = req.query("moviehash", h);
    }
    let langs = lang.split([',', ' ', ';']).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(",").to_lowercase();
    if !langs.is_empty() {
        req = req.query("languages", &langs);
    }
    if hash.is_none() {
        req = req.query("query", query);
    }
    let v = json_of(req.call().map_err(err)?)?;
    let mut out = Vec::new();
    for item in v["data"].as_array().into_iter().flatten() {
        let at = &item["attributes"];
        let Some(file) = at["files"].get(0) else { continue };
        let Some(file_id) = file["file_id"].as_i64() else { continue };
        let name = at["release"].as_str().filter(|s| !s.is_empty()).or(file["file_name"].as_str()).unwrap_or("subtitle").to_string();
        out.push(Hit {
            name,
            lang: at["language"].as_str().unwrap_or("").to_string(),
            file_id,
            downloads: at["download_count"].as_i64().unwrap_or(0),
            by_hash: hash.is_some(),
        });
        if out.len() >= 40 {
            break;
        }
    }
    Ok(out)
}

/// Search by file hash first (exact release match), then by title.
pub fn search(key: &str, lang: &str, path: Option<&str>, title: &str) -> Result<Vec<Hit>, String> {
    if key.trim().is_empty() {
        return Err("Enter your OpenSubtitles API key first".into());
    }
    let a = agent();
    let mut hits = Vec::new();
    if let Some(h) = path.and_then(|p| movie_hash(Path::new(p))) {
        hits = search_once(&a, key.trim(), lang, Some(&h), "")?;
    }
    let q = title_query(title);
    if hits.len() < 5 && !q.is_empty() {
        let more = search_once(&a, key.trim(), lang, None, &q)?;
        for m in more {
            if !hits.iter().any(|x| x.file_id == m.file_id) {
                hits.push(m);
            }
        }
    }
    Ok(hits)
}

/// Log in, fetch the subtitle and save it to `dest` (falling back to `fallback_dir`).
pub fn download(c: &Creds, file_id: i64, dest: PathBuf, fallback_dir: PathBuf) -> Result<PathBuf, String> {
    if c.key.trim().is_empty() || c.user.trim().is_empty() || c.pass.is_empty() {
        return Err("Downloading needs the API key, username and password".into());
    }
    let a = agent();
    let post = |url: String, token: Option<&str>, body: Value| {
        let mut r = a.post(url).header("Api-Key", c.key.trim()).header("User-Agent", UA).header("Accept", "application/json").header("Content-Type", "application/json");
        if let Some(t) = token {
            r = r.header("Authorization", format!("Bearer {t}"));
        }
        r.send(body.to_string().as_str()).map_err(err).and_then(json_of)
    };
    let login = post(format!("https://{HOST}/api/v1/login"), None, json!({"username": c.user.trim(), "password": c.pass}))?;
    let token = login["token"].as_str().ok_or("Login failed")?.to_string();
    let host = login["base_url"].as_str().filter(|s| !s.is_empty()).unwrap_or(HOST).to_string();
    let dl = post(format!("https://{host}/api/v1/download"), Some(&token), json!({"file_id": file_id}))?;
    let link = dl["link"].as_str().ok_or_else(|| dl["message"].as_str().unwrap_or("No download link").to_string())?;
    let bytes = a.get(link).header("User-Agent", UA).call().map_err(err)?.body_mut().read_to_vec().map_err(err)?;
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::write(&dest, &bytes).is_ok() {
        return Ok(dest);
    }
    let _ = std::fs::create_dir_all(&fallback_dir);
    let alt = fallback_dir.join(dest.file_name().unwrap_or_default());
    std::fs::write(&alt, &bytes).map_err(|e| e.to_string())?;
    Ok(alt)
}
