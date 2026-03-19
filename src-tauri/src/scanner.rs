use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use image_hasher::{HashAlg, HasherConfig};
use md5::{Digest, Md5};
use rayon::prelude::*;
use tauri::Emitter;
use walkdir::WalkDir;

use crate::db::Database;
use crate::models::{FileRecord, ScanProgress, ScanResult};

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "heic", "webp", "bmp", "tiff", "tif"];
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mov", "avi", "mkv", "wmv", "flv", "webm"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "flac", "aac", "ogg", "wma", "m4a"];

fn classify_extension(ext: &str) -> Option<&'static str> {
    let ext_lower = ext.to_lowercase();
    if IMAGE_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("image")
    } else if VIDEO_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("video")
    } else if AUDIO_EXTENSIONS.contains(&ext_lower.as_str()) {
        Some("audio")
    } else {
        None
    }
}

fn extract_exif_date(path: &Path) -> Option<String> {
    // EXIF data is always within the first 64KB of JPEG/TIFF files
    let mut file = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; 65536];
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);

    let exif = rexif::parse_buffer(&buf).ok()?;
    for entry in &exif.entries {
        if entry.tag == rexif::ExifTag::DateTimeOriginal
            || entry.tag == rexif::ExifTag::DateTime
        {
            let val = entry.value_more_readable.trim().to_string();
            if !val.is_empty() && val != "0000:00:00 00:00:00" {
                let normalized = val.replacen(':', "-", 2).replacen(' ', "T", 1);
                return Some(normalized);
            }
        }
    }
    None
}

fn extract_file_date(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if let Ok(modified) = metadata.modified() {
        let dt: chrono::DateTime<chrono::Utc> = modified.into();
        return Some(dt.to_rfc3339());
    }
    if let Ok(created) = metadata.created() {
        let dt: chrono::DateTime<chrono::Utc> = created.into();
        return Some(dt.to_rfc3339());
    }
    None
}

fn compute_phash(path: &Path) -> Option<i64> {
    let img = image::open(path).ok()?;
    let hasher = HasherConfig::new()
        .hash_size(8, 8)
        .hash_alg(HashAlg::Gradient)
        .to_hasher();
    let hash = hasher.hash_image(&img);
    let bytes = hash.as_bytes();
    if bytes.len() >= 8 {
        Some(i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    } else {
        None
    }
}

fn compute_md5(path: &Path) -> Option<String> {
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Md5::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let result = hasher.finalize();
    Some(format!("{:x}", result))
}

fn collect_media_files(folders: &[&str]) -> Vec<std::path::PathBuf> {
    let mut all_paths = Vec::new();
    for folder in folders {
        let folder_path = Path::new(folder);
        if !folder_path.exists() {
            continue;
        }
        for entry in WalkDir::new(folder_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path().to_path_buf();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if classify_extension(ext).is_some() {
                    all_paths.push(path);
                }
            }
        }
    }
    all_paths
}

/// Process a single file: extract metadata, pHash, MD5. Pure computation, no DB access.
fn process_file(path: &Path, project_id: &str) -> Option<FileRecord> {
    let path_str = path.to_string_lossy().to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let file_type = classify_extension(ext).unwrap_or("unknown");
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let file_size = fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);

    let taken_at = if file_type == "image" {
        extract_exif_date(path).or_else(|| extract_file_date(path))
    } else {
        extract_file_date(path)
    };

    let phash = if file_type == "image" {
        compute_phash(path)
    } else {
        None
    };

    let md5 = compute_md5(path);

    Some(FileRecord {
        id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.to_string(),
        path: path_str,
        file_name,
        file_size,
        file_type: file_type.to_string(),
        taken_at,
        phash,
        md5,
        group_id: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    })
}

fn scan_paths(
    db: &Database,
    project_id: &str,
    all_paths: &[std::path::PathBuf],
    window: &tauri::Window,
) -> Result<ScanResult, String> {
    let total = all_paths.len() as u64;

    // Step 1: Batch check existing paths from DB (single query, fast)
    let existing = db
        .get_existing_paths(project_id)
        .map_err(|e| e.to_string())?;

    let mut new_paths: Vec<&std::path::PathBuf> = Vec::new();
    let mut images = 0u64;
    let mut videos = 0u64;
    let mut audios = 0u64;

    for path in all_paths {
        let path_str = path.to_string_lossy().to_string();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match classify_extension(ext) {
                Some("image") => images += 1,
                Some("video") => videos += 1,
                Some("audio") => audios += 1,
                _ => {}
            }
        }
        if !existing.contains(&path_str) {
            new_paths.push(path);
        }
    }

    if new_paths.is_empty() {
        let _ = window.emit(
            "scan-progress",
            ScanProgress {
                total,
                scanned: total,
                current_file: String::new(),
            },
        );
        return Ok(ScanResult {
            total_files: total,
            images,
            videos,
            audios,
        });
    }

    let _ = window.emit(
        "scan-progress",
        ScanProgress {
            total: new_paths.len() as u64,
            scanned: 0,
            current_file: "开始并行处理...".to_string(),
        },
    );

    // Step 2: Parallel processing of new files (EXIF + pHash + MD5)
    let progress_counter = AtomicU64::new(0);
    let new_total = new_paths.len() as u64;

    let records: Vec<FileRecord> = new_paths
        .par_iter()
        .filter_map(|path| {
            let record = process_file(path, project_id);
            let done = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if done % 5 == 0 || done == new_total {
                let _ = window.emit(
                    "scan-progress",
                    ScanProgress {
                        total: new_total,
                        scanned: done,
                        current_file: path.to_string_lossy().to_string(),
                    },
                );
            }
            record
        })
        .collect();

    // Step 3: Batch insert into DB
    if let Err(e) = db.insert_files_batch(&records) {
        log::warn!("Batch insert error: {}", e);
    }

    Ok(ScanResult {
        total_files: total,
        images,
        videos,
        audios,
    })
}

pub fn scan_project(
    db: &Database,
    project_id: &str,
    source_folders: &[String],
    window: &tauri::Window,
) -> Result<ScanResult, String> {
    let folder_refs: Vec<&str> = source_folders.iter().map(|s| s.as_str()).collect();
    let all_paths = collect_media_files(&folder_refs);
    scan_paths(db, project_id, &all_paths, window)
}

pub fn scan_folder(
    db: &Database,
    project_id: &str,
    folder: &str,
    window: &tauri::Window,
) -> Result<ScanResult, String> {
    let all_paths = collect_media_files(&[folder]);
    scan_paths(db, project_id, &all_paths, window)
}
