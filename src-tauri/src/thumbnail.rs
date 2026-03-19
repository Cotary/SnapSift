use std::io::Cursor;
use std::sync::Mutex;

use base64::Engine;
use image::ImageFormat;
use lru::LruCache;

static THUMBNAIL_CACHE: std::sync::LazyLock<Mutex<LruCache<String, String>>> =
    std::sync::LazyLock::new(|| {
        Mutex::new(LruCache::new(std::num::NonZeroUsize::new(500).unwrap()))
    });

pub fn generate_thumbnail(path: &str, max_size: u32) -> Result<String, String> {
    let cache_key = format!("{}:{}", path, max_size);

    {
        let mut cache = THUMBNAIL_CACHE.lock().unwrap();
        if let Some(cached) = cache.get(&cache_key) {
            return Ok(cached.clone());
        }
    }

    let img = image::open(path).map_err(|e| format!("Failed to open image: {}", e))?;
    let thumbnail = img.thumbnail(max_size, max_size);

    let mut buf = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut buf, ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode thumbnail: {}", e))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    let data_uri = format!("data:image/jpeg;base64,{}", b64);

    {
        let mut cache = THUMBNAIL_CACHE.lock().unwrap();
        cache.put(cache_key, data_uri.clone());
    }

    Ok(data_uri)
}
