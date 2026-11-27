//! On-disk + in-memory cache for remote TikTok images (avatars, gift art,
//! emotes, and colour emoji).
//!
//! A small pool of background worker threads downloads each URL once with a
//! proper `User-Agent`/`Referer` (TikTok's CDN rejects some anonymous requests)
//! and writes the bytes to `%APPDATA%\TikTokStreamTool\image-cache\<sha256>`.
//! The UI thread turns the bytes into an egui `bytes://` URI, so decoding and
//! (GIF/WebP) animation are handled by the egui_extras loaders. Nothing is
//! drawn until the real image arrives — there are no placeholder images.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// Number of parallel download workers.
const WORKERS: usize = 2;
/// How many loaded URLs to keep mapped in memory. A long stream can show
/// thousands of distinct avatars; without a ceiling the decoded textures would
/// grow for the whole broadcast. 512 is well above the 300 chat lines on
/// screen, and evicted images are re-fetched from the disk cache if needed.
const MAX_ENTRIES: usize = 512;
/// Ceiling for the on-disk cache. Oldest files are pruned first.
const MAX_DISK_BYTES: u64 = 256 * 1024 * 1024;

pub struct ImageCache {
    request_tx: Vec<Sender<String>>,
    next_worker: usize,
    result_rx: Receiver<(String, Result<Vec<u8>, String>)>,
    /// url -> `bytes://` uri, or `None` when the load failed.
    entries: HashMap<String, Option<String>>,
    /// Insertion order of `entries`, for evicting the oldest.
    order: VecDeque<String>,
    in_flight: HashSet<String>,
}

impl ImageCache {
    pub fn new() -> Self {
        let dir = cache_dir();
        let (result_tx, result_rx) = mpsc::channel::<(String, Result<Vec<u8>, String>)>();
        let mut request_tx = Vec::with_capacity(WORKERS);
        for n in 0..WORKERS {
            let (tx, rx) = mpsc::channel::<String>();
            let worker_dir = dir.clone();
            let worker_tx = result_tx.clone();
            std::thread::Builder::new()
                .name(format!("image-cache-{n}"))
                .spawn(move || worker(rx, worker_tx, worker_dir))
                .ok();
            request_tx.push(tx);
        }
        drop(result_tx);

        // Trim the on-disk cache off the UI thread so start-up is never blocked.
        if let Some(dir) = dir {
            let _ = std::thread::Builder::new()
                .name("image-cache-prune".to_string())
                .spawn(move || prune_disk_cache(&dir, MAX_DISK_BYTES));
        }

        Self {
            request_tx,
            next_worker: 0,
            result_rx,
            entries: HashMap::new(),
            order: VecDeque::new(),
            in_flight: HashSet::new(),
        }
    }

    /// Remember a result and keep the map bounded, evicting oldest-first.
    /// Evicted images are also forgotten by egui so their decoded pixels and
    /// GPU texture are actually released, not just removed from our map.
    fn remember(&mut self, ctx: &egui::Context, url: String, uri: Option<String>) {
        if self.entries.insert(url.clone(), uri).is_none() {
            self.order.push_back(url);
        }
        for evicted in cap_entries(&mut self.entries, &mut self.order, MAX_ENTRIES) {
            ctx.forget_image(&evicted);
        }
    }

    /// The `bytes://` URI for a URL, or `None` while it is still loading.
    /// Calling this triggers a background download on first use.
    pub fn uri(&mut self, url: &str) -> Option<String> {
        if let Some(entry) = self.entries.get(url) {
            return entry.clone();
        }
        if !self.in_flight.contains(url) && !self.request_tx.is_empty() {
            self.in_flight.insert(url.to_string());
            let worker = self.next_worker % self.request_tx.len();
            self.next_worker = self.next_worker.wrapping_add(1);
            let _ = self.request_tx[worker].send(url.to_string());
        }
        None
    }

    /// Drain completed downloads and register their bytes with egui.
    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok((url, result)) = self.result_rx.try_recv() {
            self.in_flight.remove(&url);
            match result {
                Ok(bytes) if !bytes.is_empty() => {
                    let ext = sniff_ext(&bytes);
                    let uri = format!("bytes://tk/{}.{ext}", hash_url(&url));
                    ctx.include_bytes(uri.clone(), bytes);
                    self.remember(ctx, url, Some(uri));
                }
                _ => {
                    self.remember(ctx, url, None);
                }
            }
            ctx.request_repaint();
        }
        if !self.in_flight.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }

    /// Drain completed downloads while the window is minimized without
    /// registering decoded images. Bytes are already in the on-disk cache and
    /// will be re-read on restore, so the hidden UI stays small.
    pub fn poll_suspended(&mut self) {
        while let Ok((url, _)) = self.result_rx.try_recv() {
            self.in_flight.remove(&url);
        }
    }

    /// Release every decoded image and its egui texture. Live downloads and
    /// the on-disk cache are left intact.
    pub fn release_memory(&mut self, ctx: &egui::Context) {
        for (_, uri) in self.entries.drain() {
            if let Some(uri) = uri {
                ctx.forget_image(&uri);
            }
        }
        self.entries.shrink_to_fit();
        self.order.clear();
        self.order.shrink_to_fit();
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

fn worker(
    rx: Receiver<String>,
    tx: Sender<(String, Result<Vec<u8>, String>)>,
    dir: Option<PathBuf>,
) {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0 Safari/537.36")
        .timeout(Duration::from_secs(20))
        .build();
    let Ok(client) = client else {
        return;
    };
    while let Ok(url) = rx.recv() {
        let result = fetch(&client, dir.as_deref(), &url);
        if tx.send((url, result)).is_err() {
            break;
        }
    }
}

fn fetch(
    client: &reqwest::blocking::Client,
    dir: Option<&std::path::Path>,
    url: &str,
) -> Result<Vec<u8>, String> {
    let path = dir.map(|d| d.join(hash_url(url)));
    if let Some(path) = &path
        && let Ok(bytes) = std::fs::read(path)
        && !bytes.is_empty()
    {
        return Ok(bytes);
    }
    let response = client
        .get(url)
        .header(reqwest::header::REFERER, "https://www.tiktok.com/")
        .header(
            reqwest::header::ACCEPT,
            "image/avif,image/webp,image/*,*/*;q=0.8",
        )
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let bytes = response.bytes().map_err(|e| e.to_string())?.to_vec();
    if let Some(path) = &path
        && !bytes.is_empty()
    {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &bytes);
    }
    Ok(bytes)
}

/// Drop the oldest keys until at most `max` remain. Returns the URIs that were
/// evicted so the caller can release their egui images/textures.
fn cap_entries(
    entries: &mut HashMap<String, Option<String>>,
    order: &mut VecDeque<String>,
    max: usize,
) -> Vec<String> {
    let mut evicted = Vec::new();
    while order.len() > max {
        if let Some(oldest) = order.pop_front()
            && let Some(Some(uri)) = entries.remove(&oldest)
        {
            evicted.push(uri);
        }
    }
    evicted
}

/// Delete the oldest cached files until the directory fits inside `max_bytes`.
/// Runs on its own thread at start-up, so a huge cache never delays the UI.
fn prune_disk_cache(dir: &std::path::Path, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((
                meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                meta.len(),
                entry.path(),
            ))
        })
        .collect();

    let total: u64 = files.iter().map(|(_, len, _)| len).sum();
    if total <= max_bytes {
        return;
    }
    let mut freed = 0u64;
    let target = total - max_bytes;
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in files {
        if freed >= target {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            freed += len;
        }
    }
    tracing::info!("image cache pruned: freed {freed} bytes of {total}");
}

fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").map(PathBuf::from)?;
    let dir = base.join("TikTokStreamTool").join("image-cache");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

fn hash_url(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    let mut out = String::with_capacity(32);
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Sniff the image type from its magic bytes so egui picks the right decoder
/// (and animates GIFs).
fn sniff_ext(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "jpg"
    } else if bytes.starts_with(b"GIF8") {
        "gif"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        "png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_common_image_types() {
        assert_eq!(sniff_ext(&[0x89, b'P', b'N', b'G', 0, 0]), "png");
        assert_eq!(sniff_ext(&[0xFF, 0xD8, 0xFF]), "jpg");
        assert_eq!(sniff_ext(b"GIF89a"), "gif");
        assert_eq!(sniff_ext(b"RIFF\0\0\0\0WEBP\0"), "webp");
    }

    #[test]
    fn hash_is_stable_and_url_specific() {
        assert_eq!(hash_url("https://a/x"), hash_url("https://a/x"));
        assert_ne!(hash_url("https://a/x"), hash_url("https://a/y"));
    }

    #[test]
    fn the_memory_map_keeps_only_the_newest_entries() {
        let mut entries: HashMap<String, Option<String>> = HashMap::new();
        let mut order: VecDeque<String> = VecDeque::new();
        for i in 0..10 {
            let url = format!("https://cdn/{i}.png");
            if entries.insert(url.clone(), Some(url.clone())).is_none() {
                order.push_back(url);
            }
            cap_entries(&mut entries, &mut order, 3);
        }
        assert_eq!(entries.len(), 3);
        // The three newest survive; the oldest are gone.
        assert!(entries.contains_key("https://cdn/9.png"));
        assert!(entries.contains_key("https://cdn/8.png"));
        assert!(entries.contains_key("https://cdn/7.png"));
        assert!(!entries.contains_key("https://cdn/0.png"));
    }

    #[test]
    fn eviction_reports_uris_for_release() {
        let mut entries: HashMap<String, Option<String>> = HashMap::new();
        let mut order: VecDeque<String> = VecDeque::new();
        for i in 0..5 {
            let url = format!("https://cdn/{i}.png");
            entries.insert(url.clone(), Some(format!("bytes://tk/{i}.png")));
            order.push_back(url);
        }
        let evicted = cap_entries(&mut entries, &mut order, 2);
        assert_eq!(entries.len(), 2);
        assert_eq!(evicted.len(), 3);
        assert!(evicted.contains(&"bytes://tk/0.png".to_string()));
        assert!(evicted.contains(&"bytes://tk/2.png".to_string()));
        assert!(!evicted.contains(&"bytes://tk/4.png".to_string()));
    }

    #[test]
    fn pruning_removes_the_oldest_files_first() {
        let dir = tempfile::tempdir().unwrap();
        let handle = |name: &str, age_secs: u64| {
            let path = dir.path().join(name);
            std::fs::write(&path, vec![0u8; 100]).unwrap();
            let when = std::time::SystemTime::now() - Duration::from_secs(age_secs);
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        handle("oldest.png", 300);
        handle("middle.png", 200);
        handle("newest.png", 100);

        // 300 bytes total, budget 150 -> the oldest 200 bytes must go first.
        prune_disk_cache(dir.path(), 150);

        let left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(left.contains(&"newest.png".to_string()), "got {left:?}");
        assert!(
            left.len() < 3,
            "expected the oldest files to be pruned: {left:?}"
        );
    }
}
