//! Image loading & caching for inline/standalone images (UI-MD-010).
//!
//! Local images are decoded synchronously (small files) and cached in an egui
//! texture. Remote images are fetched on a background thread so the UI thread
//! is never blocked; a placeholder is drawn until the bytes arrive. Caching is
//! bounded by a simple key set so repeated images do not re-download.

use egui::{ColorImage, Context, TextureHandle, TextureOptions, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum number of decoded remote images kept in the texture cache.
const MAX_REMOTE_CACHE: usize = 64;

/// Completed background fetch results shared with the UI thread.
type Received = Arc<Mutex<Vec<(String, Result<Vec<u8>, String>)>>>;

struct Pending {
    received: Received,
    in_flight: HashSet<String>,
}

pub struct ImageStore {
    base_dir: PathBuf,
    /// resolved-normalized source -> decoded texture + natural size (px).
    cache: HashMap<String, (TextureHandle, Vec2)>,
    pending: Pending,
}

impl ImageStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            cache: HashMap::new(),
            pending: Pending {
                received: Arc::new(Mutex::new(Vec::new())),
                in_flight: HashSet::new(),
            },
        }
    }

    /// Absolute path a local `src` resolves to (if it is a local relative/absolute path).
    fn resolve_local(&self, src: &str) -> Option<PathBuf> {
        let stripped = src.split(['#', '?']).next().unwrap_or(src);
        let s = stripped.trim();
        if s.is_empty()
            || s.starts_with("data:")
            || s.starts_with("http://")
            || s.starts_with("https://")
            || s.starts_with("//")
        {
            return None;
        }
        let p = Path::new(s);
        if p.is_absolute() {
            Some(p.to_path_buf())
        } else {
            Some(self.base_dir.join(p))
        }
    }

    fn is_remote(src: &str) -> bool {
        src.starts_with("http://") || src.starts_with("https://")
    }

    /// Drain completed background fetches, decoding and caching any that arrived.
    /// Call once per frame.
    pub fn poll(&mut self, ctx: &Context) {
        let received = {
            let mut guard = self.pending.received.lock().unwrap();
            std::mem::take(&mut *guard)
        };
        if received.is_empty() {
            return;
        }
        for (key, result) in received {
            self.pending.in_flight.remove(&key);
            match result {
                Ok(bytes) => {
                    if let Some(ci) = decode_color_image(&bytes) {
                        let size = Vec2::new(ci.size[0] as f32, ci.size[1] as f32);
                        let handle = ctx.load_texture(&key, ci, TextureOptions::LINEAR);
                        self.insert_bounded(&key, handle, size);
                    }
                }
                Err(_) => {
                    // Failed download: remember nothing; will retry on next access.
                }
            }
        }
    }

    fn insert_bounded(&mut self, key: &str, handle: TextureHandle, size: Vec2) {
        self.cache.insert(key.to_string(), (handle, size));
        if self.cache.len() > MAX_REMOTE_CACHE {
            // Evict the first (oldest) entry.
            if let Some(k) = self.cache.keys().next().cloned() {
                self.cache.remove(&k);
            }
        }
    }

    /// Ensure a background fetch is scheduled for a remote image (idempotent).
    fn schedule_remote(&mut self, src: &str, ctx: &Context) {
        if self.pending.in_flight.contains(src) {
            return;
        }
        self.pending.in_flight.insert(src.to_string());
        let key = src.to_string();
        let received = Arc::clone(&self.pending.received);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = fetch_remote(&key);
            received.lock().unwrap().push((key, result));
            // Wake the UI so the completed image is decoded and shown promptly.
            ctx.request_repaint();
        });
    }

    /// Return a texture for `src`, performing any non-blocking state transition.
    /// Returns `(texture, natural_size)` when an image is already available.
    pub fn texture_for(&mut self, ctx: &Context, src: &str) -> Option<(TextureHandle, Vec2)> {
        self.poll(ctx);
        if let Some((h, s)) = self.cache.get(src) {
            return Some((h.clone(), *s));
        }
        if let Some(path) = self.resolve_local(src) {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Some(ci) = decode_color_image(&bytes) {
                    let size = Vec2::new(ci.size[0] as f32, ci.size[1] as f32);
                    let handle = ctx.load_texture(src, ci, TextureOptions::LINEAR);
                    self.insert_bounded(src, handle.clone(), size);
                    return Some((handle, size));
                }
            }
            return None;
        }
        if Self::is_remote(src) {
            self.schedule_remote(src, ctx);
        }
        None
    }

    /// Whether the image is still loading (so the caller can show a spinner label).
    pub fn is_pending(&self, src: &str) -> bool {
        self.pending.in_flight.contains(src)
    }
}

fn decode_color_image(bytes: &[u8]) -> Option<ColorImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let size = [w as usize, h as usize];
    Some(ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

#[cfg(feature = "remote-images")]
fn fetch_remote(url: &str) -> Result<Vec<u8>, String> {
    use std::time::Duration;
    use ureq::tls::{TlsConfig, TlsProvider};
    let config = ureq::config::Config::builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_recv_body(Some(Duration::from_secs(15)))
        .tls_config(TlsConfig::builder().provider(TlsProvider::Rustls).build())
        .build();
    let agent = ureq::Agent::new_with_config(config);
    agent
        .get(url)
        .call()
        .map_err(|e| e.to_string())?
        .into_body()
        .read_to_vec()
        .map_err(|e| e.to_string())
}

#[cfg(not(feature = "remote-images"))]
fn fetch_remote(url: &str) -> Result<Vec<u8>, String> {
    let _ = url;
    Err("remote images disabled".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_urls_are_not_treated_as_local_paths() {
        let store = ImageStore::new(PathBuf::from("/tmp"));
        assert!(store.resolve_local("https://example.com/a.png").is_none());
        assert!(store.resolve_local("http://example.com/a.png").is_none());
        assert!(store.resolve_local("//example.com/a.png").is_none());
        assert!(store.resolve_local("data:image/png;base64,AAAA").is_none());
        assert!(store.resolve_local("img.png").is_some());
        assert!(store.resolve_local("/abs/img.png").is_some());
    }
}
