use std::fs;
use std::path::{Path, PathBuf};

use tauri::Emitter;

use crate::models::{FileRecord, OrganizeProgress, OrganizeResult};

fn parse_date_components(taken_at: &str) -> Option<(String, String, String)> {
    // Handle both "YYYY-MM-DDTHH:MM:SS" and "YYYY-MM-DD HH:MM:SS" formats
    let date_part = taken_at.split('T').next().unwrap_or(taken_at);
    let date_part = date_part.split(' ').next().unwrap_or(date_part);

    let parts: Vec<&str> = date_part.split('-').collect();
    if parts.len() >= 3 {
        let year = parts[0].to_string();
        let month = parts[1].to_string();
        let day = parts[2].to_string();
        return Some((year, month, day));
    }
    None
}

fn apply_pattern(pattern: &str, year: &str, month: &str, day: &str) -> String {
    pattern
        .replace("YYYY", year)
        .replace("MM", month)
        .replace("DD", day)
}

fn resolve_conflict(target: &Path) -> PathBuf {
    if !target.exists() {
        return target.to_path_buf();
    }

    let stem = target
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    let ext = target
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let parent = target.parent().unwrap_or(Path::new("."));

    loop {
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
        let new_name = if ext.is_empty() {
            format!("{}_{}", stem, suffix)
        } else {
            format!("{}_{}.{}", stem, suffix, ext)
        };
        let candidate = parent.join(new_name);
        if !candidate.exists() {
            return candidate;
        }
    }
}

pub fn organize_files(
    files: &[FileRecord],
    existing_target_files: &[FileRecord],
    target_dir: &str,
    pattern: &str,
    mode: &str,
    file_types: &[String],
    window: &tauri::Window,
) -> Result<OrganizeResult, String> {
    let filtered: Vec<&FileRecord> = files
        .iter()
        .filter(|f| file_types.is_empty() || file_types.contains(&f.file_type))
        .collect();
    let total = filtered.len() as u64;
    let mut processed = 0u64;
    let mut success = 0u64;
    let mut skipped = 0u64;

    let target_base = Path::new(target_dir);

    // Pre-populate with MD5s of files that actually exist on disk in target folder
    let mut seen_md5: std::collections::HashSet<String> = std::collections::HashSet::new();
    for ef in existing_target_files {
        if let Some(ref md5) = ef.md5 {
            if Path::new(&ef.path).exists() {
                seen_md5.insert(md5.clone());
            }
        }
    }

    for file in &filtered {
        let source = Path::new(&file.path);
        if !source.exists() {
            skipped += 1;
            processed += 1;
            continue;
        }

        // Skip files with the same content (MD5) already organized to target
        if let Some(ref md5) = file.md5 {
            if !seen_md5.insert(md5.clone()) {
                skipped += 1;
                processed += 1;
                continue;
            }
        }

        let taken_at = match &file.taken_at {
            Some(t) => t.clone(),
            None => {
                skipped += 1;
                processed += 1;
                continue;
            }
        };

        let (year, month, day) = match parse_date_components(&taken_at) {
            Some(components) => components,
            None => {
                skipped += 1;
                processed += 1;
                continue;
            }
        };

        let sub_dir = apply_pattern(pattern, &year, &month, &day);
        let dest_dir = target_base.join(&sub_dir);

        if let Err(e) = fs::create_dir_all(&dest_dir) {
            log::warn!("Failed to create directory {:?}: {}", dest_dir, e);
            skipped += 1;
            processed += 1;
            continue;
        }

        let dest_file = dest_dir.join(&file.file_name);
        let final_dest = resolve_conflict(&dest_file);

        let result = match mode {
            "move" => fs::rename(source, &final_dest).or_else(|_| {
                // rename fails across drives; fallback to copy+delete
                fs::copy(source, &final_dest).and_then(|_| fs::remove_file(source))
            }),
            _ => fs::copy(source, &final_dest).map(|_| ()),
        };

        match result {
            Ok(_) => success += 1,
            Err(e) => {
                log::warn!("Failed to {} file {:?}: {}", mode, source, e);
                skipped += 1;
            }
        }

        processed += 1;

        if processed % 5 == 0 || processed == total {
            let _ = window.emit(
                "organize-progress",
                OrganizeProgress {
                    total,
                    processed,
                    current_file: file.file_name.clone(),
                },
            );
        }
    }

    Ok(OrganizeResult {
        total,
        success,
        skipped,
    })
}
