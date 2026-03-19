use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;
use tauri::Emitter;

use crate::db::Database;
use crate::embedder;
use crate::models::{DedupProgress, DedupResult, StageTiming};

fn hamming_distance(a: i64, b: i64) -> u32 {
    (a ^ b).count_ones()
}

const DEFAULT_PHASH_THRESHOLD: u32 = 8;
const DEFAULT_COSINE_THRESHOLD: f32 = 0.93;
const MAX_GROUP_SIZE: usize = 20;

struct DedupConfig {
    phash_threshold: u32,
    phash_suspect_max: u32,
    cosine_threshold: f32,
    max_ingroup_phash: u32,
}

impl DedupConfig {
    fn new(phash_threshold: u32, cosine_threshold: f32) -> Self {
        Self {
            phash_threshold,
            phash_suspect_max: phash_threshold + 4,
            cosine_threshold,
            max_ingroup_phash: phash_threshold + 4,
        }
    }
}

// ---------------------------------------------------------------------------
// Complete-Linkage clustering
// ---------------------------------------------------------------------------

struct Clusters {
    members: Vec<Vec<usize>>,
    index: Vec<usize>,
}

impl Clusters {
    fn new(n: usize) -> Self {
        Self {
            members: (0..n).map(|i| vec![i]).collect(),
            index: (0..n).collect(),
        }
    }

    fn cluster_of(&self, i: usize) -> usize {
        self.index[i]
    }

    fn try_merge<F>(&mut self, a: usize, b: usize, is_similar: F) -> bool
    where
        F: Fn(usize, usize) -> bool,
    {
        let ca = self.index[a];
        let cb = self.index[b];
        if ca == cb {
            return false;
        }

        let (smaller_id, larger_id) = if self.members[ca].len() <= self.members[cb].len() {
            (ca, cb)
        } else {
            (cb, ca)
        };

        for &m_small in &self.members[smaller_id] {
            for &m_large in &self.members[larger_id] {
                if !is_similar(m_small, m_large) {
                    return false;
                }
            }
        }

        let small_members: Vec<usize> = std::mem::take(&mut self.members[smaller_id]);
        for &m in &small_members {
            self.index[m] = larger_id;
        }
        self.members[larger_id].extend(small_members);
        true
    }

    fn get_groups(&self) -> Vec<Vec<usize>> {
        let mut seen = std::collections::HashSet::new();
        let mut groups = Vec::new();
        for c in &self.index {
            if self.members[*c].len() >= 2 && seen.insert(*c) {
                groups.push(self.members[*c].clone());
            }
        }
        groups
    }
}

pub fn find_duplicates(
    db: &Database,
    project_id: &str,
    phash_threshold: Option<u32>,
    cosine_threshold: Option<f32>,
    target_dir: Option<&str>,
    window: &tauri::Window,
) -> Result<DedupResult, String> {
    let cfg = DedupConfig::new(
        phash_threshold.unwrap_or(DEFAULT_PHASH_THRESHOLD),
        cosine_threshold.unwrap_or(DEFAULT_COSINE_THRESHOLD),
    );

    db.clear_group_ids(project_id)
        .map_err(|e| e.to_string())?;

    let raw_files = db
        .get_files_with_phash(project_id, target_dir)
        .map_err(|e| e.to_string())?;

    let files = {
        let mut seen = std::collections::HashSet::new();
        raw_files
            .into_iter()
            .filter(|f| seen.insert(f.path.replace('\\', "/").to_lowercase()))
            .collect::<Vec<_>>()
    };

    let total_target_images = files.len() as u64;

    if files.is_empty() {
        return Ok(DedupResult {
            total_target_images: 0,
            groups_found: 0,
            total_duplicates: 0,
            methods: vec!["pHash".to_string()],
            suspect_pairs_checked: 0,
            ai_confirmed: 0,
            timings: vec![],
            total_duration_ms: 0,
        });
    }

    let total_start = std::time::Instant::now();
    let mut timings: Vec<StageTiming> = Vec::new();

    let emit_progress = |stage: &str, current: u64, total: u64, file: &str| {
        let _ = window.emit(
            "dedup-progress",
            DedupProgress {
                stage: stage.to_string(),
                current,
                total,
                current_file: file.to_string(),
            },
        );
    };

    let n = files.len();
    let ai_available = embedder::is_available();

    let mut methods = vec!["pHash".to_string()];
    if ai_available {
        methods.push("AI向量(MobileNet-v3)".to_string());
    }

    // Pre-compute pHash array for fast access
    let hashes: Vec<i64> = files.iter().map(|f| f.phash.unwrap()).collect();

    // -----------------------------------------------------------------------
    // Stage 0: MD5 exact duplicate grouping
    // -----------------------------------------------------------------------
    let stage_start = std::time::Instant::now();
    let mut clusters = Clusters::new(n);
    {
        let mut md5_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for i in 0..n {
            if let Some(ref md5) = files[i].md5 {
                if let Some(&first) = md5_map.get(md5) {
                    // MD5 identical → always merge (skip complete-linkage check)
                    let ca = clusters.cluster_of(first);
                    let cb = clusters.cluster_of(i);
                    if ca != cb {
                        let small_members: Vec<usize> =
                            std::mem::take(&mut clusters.members[cb]);
                        for &m in &small_members {
                            clusters.index[m] = ca;
                        }
                        clusters.members[ca].extend(small_members);
                    }
                } else {
                    md5_map.insert(md5.clone(), i);
                }
            }
        }
    }
    timings.push(StageTiming {
        name: "MD5 去重".into(),
        duration_ms: stage_start.elapsed().as_millis() as u64,
    });

    // -----------------------------------------------------------------------
    // Stage 1: pHash comparison with Complete-Linkage
    // -----------------------------------------------------------------------
    emit_progress("pHash 比较", 0, n as u64, "");
    let stage_start = std::time::Instant::now();

    let mut phash_pairs: Vec<(usize, usize)> = Vec::new();
    let mut suspect_pairs: Vec<(usize, usize)> = Vec::new();

    for i in 0..n {
        for j in (i + 1)..n {
            if clusters.cluster_of(i) == clusters.cluster_of(j) {
                continue;
            }
            let dist = hamming_distance(hashes[i], hashes[j]);
            if dist <= cfg.phash_threshold {
                phash_pairs.push((i, j));
            } else if ai_available && dist <= cfg.phash_suspect_max {
                suspect_pairs.push((i, j));
            }
        }
        if (i + 1) % 10 == 0 || i + 1 == n {
            emit_progress("pHash 比较", (i + 1) as u64, n as u64, &files[i].file_name);
        }
    }

    // Sort pairs by distance (tightest first) for better cluster formation
    phash_pairs.sort_by_key(|&(a, b)| hamming_distance(hashes[a], hashes[b]));

    for &(i, j) in &phash_pairs {
        let h = &hashes;
        clusters.try_merge(i, j, |a, b| hamming_distance(h[a], h[b]) <= cfg.phash_threshold);
    }

    timings.push(StageTiming {
        name: "pHash 比较".into(),
        duration_ms: stage_start.elapsed().as_millis() as u64,
    });

    // -----------------------------------------------------------------------
    // Stage 2: AI feature extraction + verification/confirmation
    // -----------------------------------------------------------------------
    let stage_start = std::time::Instant::now();
    let mut ai_confirmed = 0u64;
    let mut suspect_count = 0u64;

    if ai_available {
        // 2a: Load / extract AI vectors
        emit_progress("AI 特征提取", 0, n as u64, "加载缓存...");
        let ai_extract_start = std::time::Instant::now();

        let mut vectors: Vec<Option<Vec<f32>>> = vec![None; n];
        let mut uncached_indices: Vec<usize> = Vec::new();

        // Determine which files need vectors: group members + suspect pair participants
        let mut need_vector = vec![false; n];
        for group in clusters.get_groups() {
            for &m in &group {
                need_vector[m] = true;
            }
        }
        for &(a, b) in &suspect_pairs {
            need_vector[a] = true;
            need_vector[b] = true;
        }

        for i in 0..n {
            if !need_vector[i] {
                continue;
            }
            if let Ok(Some(bytes)) = db.get_feature_vector(&files[i].id) {
                if !bytes.is_empty() {
                    vectors[i] = Some(embedder::bytes_to_feature(&bytes));
                    continue;
                }
            }
            uncached_indices.push(i);
        }

        let cached_count = need_vector.iter().filter(|&&v| v).count() - uncached_indices.len();
        if cached_count > 0 {
            log::info!(
                "AI vectors: {} cached, {} to extract",
                cached_count,
                uncached_indices.len()
            );
        }

        if !uncached_indices.is_empty() {
            let progress_counter = AtomicU64::new(0);
            let total_uncached = uncached_indices.len() as u64;
            let total_need = need_vector.iter().filter(|&&v| v).count() as u64;

            let extracted: Vec<(usize, Option<Vec<f32>>)> = uncached_indices
                .par_iter()
                .map(|&i| {
                    let v = embedder::extract_feature(&files[i].path);
                    let done = progress_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if done % 5 == 0 || done == total_uncached {
                        emit_progress(
                            "AI 特征提取(多核)",
                            cached_count as u64 + done,
                            total_need,
                            &files[i].file_name,
                        );
                    }
                    (i, v)
                })
                .collect();

            for (i, v) in extracted {
                if let Some(ref vec) = v {
                    let bytes = embedder::feature_to_bytes(vec);
                    db.set_feature_vector(&files[i].id, &bytes).ok();
                }
                vectors[i] = v;
            }
        }

        timings.push(StageTiming {
            name: "AI 特征提取".into(),
            duration_ms: ai_extract_start.elapsed().as_millis() as u64,
        });

        // 2b: Verify existing groups — kick out members with low AI similarity
        emit_progress("AI 验证分组", 0, n as u64, "");
        let ai_verify_start = std::time::Instant::now();
        {
            let current_groups = clusters.get_groups();
            for group in &current_groups {
                if group.len() < 2 {
                    continue;
                }
                let mut to_evict: Vec<usize> = Vec::new();
                for &m in group {
                    let vi = match &vectors[m] {
                        Some(v) => v,
                        None => {
                            to_evict.push(m);
                            continue;
                        }
                    };
                    let mut min_sim = f32::MAX;
                    for &other in group {
                        if other == m {
                            continue;
                        }
                        if let Some(ref vj) = vectors[other] {
                            let sim = embedder::cosine_similarity(vi, vj);
                            if sim < min_sim {
                                min_sim = sim;
                            }
                        }
                    }
                    if min_sim < cfg.cosine_threshold && min_sim < f32::MAX {
                        to_evict.push(m);
                    }
                }
                for &m in &to_evict {
                    let cid = clusters.cluster_of(m);
                    if clusters.members[cid].len() <= 1 {
                        continue;
                    }
                    clusters.members[cid].retain(|&x| x != m);
                    let new_cid = clusters.members.len();
                    clusters.members.push(vec![m]);
                    clusters.index[m] = new_cid;
                }
            }
        }
        timings.push(StageTiming {
            name: "AI 验证分组".into(),
            duration_ms: ai_verify_start.elapsed().as_millis() as u64,
        });

        // 2c: Confirm suspect pHash pairs using AI cosine similarity
        emit_progress("AI 确认疑似对", 0, suspect_pairs.len() as u64, "");
        let ai_confirm_start = std::time::Instant::now();

        suspect_count = suspect_pairs.len() as u64;
        for (idx, &(i, j)) in suspect_pairs.iter().enumerate() {
            if clusters.cluster_of(i) == clusters.cluster_of(j) {
                continue;
            }
            if let (Some(ref vi), Some(ref vj)) = (&vectors[i], &vectors[j]) {
                let sim = embedder::cosine_similarity(vi, vj);
                if sim >= cfg.cosine_threshold {
                    let h = &hashes;
                    let v = &vectors;
                    let cos_t = cfg.cosine_threshold;
                    let sus_max = cfg.phash_suspect_max;
                    let merged = clusters.try_merge(i, j, |a, b| {
                        let phash_ok = hamming_distance(h[a], h[b]) <= sus_max;
                        let ai_ok = match (&v[a], &v[b]) {
                            (Some(va), Some(vb)) => {
                                embedder::cosine_similarity(va, vb) >= cos_t
                            }
                            _ => false,
                        };
                        phash_ok && ai_ok
                    });
                    if merged {
                        ai_confirmed += 1;
                    }
                }
            }
            if (idx + 1) % 20 == 0 || idx + 1 == suspect_pairs.len() {
                emit_progress(
                    "AI 确认疑似对",
                    (idx + 1) as u64,
                    suspect_pairs.len() as u64,
                    "",
                );
            }
        }

        timings.push(StageTiming {
            name: "AI 确认疑似对".into(),
            duration_ms: ai_confirm_start.elapsed().as_millis() as u64,
        });
    } else {
        timings.push(StageTiming {
            name: "AI (未启用)".into(),
            duration_ms: stage_start.elapsed().as_millis() as u64,
        });
    }

    // -----------------------------------------------------------------------
    // Stage 3: Post-merge validation
    // -----------------------------------------------------------------------
    let stage_start = std::time::Instant::now();
    emit_progress("组内校验", 0, n as u64, "");

    // 3a: Evict members whose worst pHash distance to any group member exceeds limit
    {
        let current_groups = clusters.get_groups();
        for group in &current_groups {
            if group.len() < 2 {
                continue;
            }
            let mut to_evict: Vec<usize> = Vec::new();
            for &m in group {
                let worst_dist = group
                    .iter()
                    .filter(|&&o| o != m)
                    .map(|&o| hamming_distance(hashes[m], hashes[o]))
                    .max()
                    .unwrap_or(0);
                if worst_dist > cfg.max_ingroup_phash {
                    to_evict.push(m);
                }
            }
            for &m in &to_evict {
                let cid = clusters.cluster_of(m);
                if clusters.members[cid].len() <= 1 {
                    continue;
                }
                clusters.members[cid].retain(|&x| x != m);
                let new_cid = clusters.members.len();
                clusters.members.push(vec![m]);
                clusters.index[m] = new_cid;
            }
        }
    }

    // 3b: Split oversized groups with stricter threshold
    {
        let current_groups = clusters.get_groups();
        for group in current_groups {
            if group.len() <= MAX_GROUP_SIZE {
                continue;
            }
            let cid = clusters.cluster_of(group[0]);
            let old_members: Vec<usize> = std::mem::take(&mut clusters.members[cid]);

            let strict_thresh = cfg.phash_threshold / 2;
            let mut sub_groups: Vec<Vec<usize>> = Vec::new();
            for &m in &old_members {
                let mut placed = false;
                for sg in &mut sub_groups {
                    let fits = sg.iter().all(|&existing| {
                        hamming_distance(hashes[m], hashes[existing]) <= strict_thresh
                    });
                    if fits {
                        sg.push(m);
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    sub_groups.push(vec![m]);
                }
            }

            // First sub-group keeps the original cluster ID
            if let Some(first_sg) = sub_groups.first() {
                clusters.members[cid] = first_sg.clone();
                for &m in first_sg {
                    clusters.index[m] = cid;
                }
            }
            for sg in sub_groups.iter().skip(1) {
                let new_cid = clusters.members.len();
                clusters.members.push(sg.clone());
                for &m in sg {
                    clusters.index[m] = new_cid;
                }
            }
        }
    }

    timings.push(StageTiming {
        name: "组内校验".into(),
        duration_ms: stage_start.elapsed().as_millis() as u64,
    });

    // -----------------------------------------------------------------------
    // Output: write group IDs to DB
    // -----------------------------------------------------------------------
    let final_groups = clusters.get_groups();
    let mut groups_found = 0u64;
    let mut total_duplicates = 0u64;

    for members in &final_groups {
        groups_found += 1;
        total_duplicates += members.len() as u64;

        let group_id = uuid::Uuid::new_v4().to_string();
        for &idx in members {
            db.update_group_id(&files[idx].id, &group_id)
                .map_err(|e| e.to_string())?;
        }
    }

    let total_duration_ms = total_start.elapsed().as_millis() as u64;

    Ok(DedupResult {
        total_target_images,
        groups_found,
        total_duplicates,
        methods,
        suspect_pairs_checked: suspect_count,
        ai_confirmed,
        timings,
        total_duration_ms,
    })
}
