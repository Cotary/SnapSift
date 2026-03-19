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

    fn force_merge(&mut self, a: usize, b: usize) {
        let ca = self.index[a];
        let cb = self.index[b];
        if ca == cb {
            return;
        }
        let small_members: Vec<usize> = std::mem::take(&mut self.members[cb]);
        for &m in &small_members {
            self.index[m] = ca;
        }
        self.members[ca].extend(small_members);
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

    fn evict(&mut self, m: usize) {
        let cid = self.cluster_of(m);
        if self.members[cid].len() <= 1 {
            return;
        }
        self.members[cid].retain(|&x| x != m);
        let new_cid = self.members.len();
        self.members.push(vec![m]);
        self.index[m] = new_cid;
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

// ---------------------------------------------------------------------------
// Shared: extract AI vectors for given indices
// ---------------------------------------------------------------------------
fn extract_vectors(
    db: &Database,
    files: &[crate::models::FileRecord],
    need_vector: &[bool],
    n: usize,
    emit_progress: &(dyn Fn(&str, u64, u64, &str) + Sync),
) -> Vec<Option<Vec<f32>>> {
    emit_progress("AI 特征提取", 0, n as u64, "");

    let mut vectors: Vec<Option<Vec<f32>>> = vec![None; n];
    let mut uncached_indices: Vec<usize> = Vec::new();

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

    vectors
}

pub fn find_duplicates(
    db: &Database,
    project_id: &str,
    mode: Option<&str>,
    phash_threshold: Option<u32>,
    cosine_threshold: Option<f32>,
    target_dir: Option<&str>,
    window: &tauri::Window,
) -> Result<DedupResult, String> {
    let mode = mode.unwrap_or("phash_ai");
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
            methods: vec![],
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

    let methods = match mode {
        "phash" => vec!["pHash".to_string()],
        "ai" => vec!["AI(MobileNet-v3)".to_string()],
        _ => {
            let mut m = vec!["pHash".to_string()];
            if ai_available {
                m.push("AI(MobileNet-v3)".to_string());
            }
            m
        }
    };

    let hashes: Vec<i64> = files.iter().map(|f| f.phash.unwrap()).collect();

    // -----------------------------------------------------------------------
    // Stage 0: MD5 exact duplicate grouping (always runs)
    // -----------------------------------------------------------------------
    let stage_start = std::time::Instant::now();
    let mut clusters = Clusters::new(n);
    {
        let mut md5_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for i in 0..n {
            if let Some(ref md5) = files[i].md5 {
                if let Some(&first) = md5_map.get(md5) {
                    clusters.force_merge(first, i);
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

    let mut ai_confirmed = 0u64;
    let mut suspect_count = 0u64;

    match mode {
        // =================================================================
        // MODE: Pure AI
        // =================================================================
        "ai" => {
            if !ai_available {
                return Err("AI engine not available. Cannot use Pure AI mode.".to_string());
            }

            // Stage A1: Extract vectors for ALL images
            let stage_start = std::time::Instant::now();
            let need_vector = vec![true; n];
            let vectors = extract_vectors(db, &files, &need_vector, n, &emit_progress);
            timings.push(StageTiming {
                name: "AI 特征提取(全量)".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });

            // Stage A2: O(n^2) cosine comparison + Complete-Linkage clustering
            let stage_start = std::time::Instant::now();
            emit_progress("AI 全量比对", 0, n as u64, "");

            let total_pairs = (n * (n - 1)) / 2;
            let mut ai_pairs: Vec<(usize, usize, f32)> = Vec::new();

            let mut pair_count = 0u64;
            for i in 0..n {
                for j in (i + 1)..n {
                    if clusters.cluster_of(i) == clusters.cluster_of(j) {
                        pair_count += 1;
                        continue;
                    }
                    if let (Some(ref vi), Some(ref vj)) = (&vectors[i], &vectors[j]) {
                        let sim = embedder::cosine_similarity(vi, vj);
                        if sim >= cfg.cosine_threshold {
                            ai_pairs.push((i, j, sim));
                        }
                    }
                    pair_count += 1;
                }
                if (i + 1) % 10 == 0 || i + 1 == n {
                    emit_progress(
                        "AI 全量比对",
                        pair_count,
                        total_pairs as u64,
                        &files[i].file_name,
                    );
                }
            }

            // Sort by similarity descending for better cluster formation
            ai_pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

            for &(i, j, _) in &ai_pairs {
                let v = &vectors;
                let cos_t = cfg.cosine_threshold;
                let merged = clusters.try_merge(i, j, |a, b| {
                    match (&v[a], &v[b]) {
                        (Some(va), Some(vb)) => embedder::cosine_similarity(va, vb) >= cos_t,
                        _ => false,
                    }
                });
                if merged {
                    ai_confirmed += 1;
                }
            }

            suspect_count = ai_pairs.len() as u64;

            timings.push(StageTiming {
                name: "AI 全量比对".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });

            // Stage A3: Split oversized groups (using cosine instead of pHash)
            let stage_start = std::time::Instant::now();
            {
                let current_groups = clusters.get_groups();
                for group in current_groups {
                    if group.len() <= MAX_GROUP_SIZE {
                        continue;
                    }
                    let cid = clusters.cluster_of(group[0]);
                    let old_members: Vec<usize> = std::mem::take(&mut clusters.members[cid]);

                    let strict_cos = cfg.cosine_threshold + (1.0 - cfg.cosine_threshold) * 0.5;
                    let mut sub_groups: Vec<Vec<usize>> = Vec::new();
                    for &m in &old_members {
                        let mut placed = false;
                        for sg in &mut sub_groups {
                            let fits = sg.iter().all(|&existing| {
                                match (&vectors[m], &vectors[existing]) {
                                    (Some(vm), Some(ve)) => {
                                        embedder::cosine_similarity(vm, ve) >= strict_cos
                                    }
                                    _ => false,
                                }
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
                name: "超大组拆分".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });
        }

        // =================================================================
        // MODE: pHash only
        // =================================================================
        "phash" => {
            // Stage 1: pHash comparison
            emit_progress("pHash 比较", 0, n as u64, "");
            let stage_start = std::time::Instant::now();

            let mut phash_pairs: Vec<(usize, usize)> = Vec::new();
            for i in 0..n {
                for j in (i + 1)..n {
                    if clusters.cluster_of(i) == clusters.cluster_of(j) {
                        continue;
                    }
                    let dist = hamming_distance(hashes[i], hashes[j]);
                    if dist <= cfg.phash_threshold {
                        phash_pairs.push((i, j));
                    }
                }
                if (i + 1) % 10 == 0 || i + 1 == n {
                    emit_progress("pHash 比较", (i + 1) as u64, n as u64, &files[i].file_name);
                }
            }

            phash_pairs.sort_by_key(|&(a, b)| hamming_distance(hashes[a], hashes[b]));
            for &(i, j) in &phash_pairs {
                let h = &hashes;
                clusters.try_merge(i, j, |a, b| {
                    hamming_distance(h[a], h[b]) <= cfg.phash_threshold
                });
            }

            timings.push(StageTiming {
                name: "pHash 比较".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });

            // Stage 3: Post-merge pHash validation
            let stage_start = std::time::Instant::now();
            emit_progress("组内校验", 0, n as u64, "");
            run_phash_postvalidation(&mut clusters, &hashes, &cfg);
            run_phash_oversized_split(&mut clusters, &hashes, &cfg);
            timings.push(StageTiming {
                name: "组内校验".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });
        }

        // =================================================================
        // MODE: pHash + AI (default, original behavior)
        // =================================================================
        _ => {
            // Stage 1: pHash comparison
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

            phash_pairs.sort_by_key(|&(a, b)| hamming_distance(hashes[a], hashes[b]));
            for &(i, j) in &phash_pairs {
                let h = &hashes;
                clusters.try_merge(i, j, |a, b| {
                    hamming_distance(h[a], h[b]) <= cfg.phash_threshold
                });
            }

            timings.push(StageTiming {
                name: "pHash 比较".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });

            // Stage 2: AI
            if ai_available {
                // 2a: Extract vectors for group members + suspect pairs
                let ai_extract_start = std::time::Instant::now();
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

                let vectors = extract_vectors(db, &files, &need_vector, n, &emit_progress);
                timings.push(StageTiming {
                    name: "AI 特征提取".into(),
                    duration_ms: ai_extract_start.elapsed().as_millis() as u64,
                });

                // 2b: Verify existing groups
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
                            clusters.evict(m);
                        }
                    }
                }
                timings.push(StageTiming {
                    name: "AI 验证分组".into(),
                    duration_ms: ai_verify_start.elapsed().as_millis() as u64,
                });

                // 2c: Confirm suspect pairs
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
                    duration_ms: 0,
                });
            }

            // Stage 3: Post-merge validation
            let stage_start = std::time::Instant::now();
            emit_progress("组内校验", 0, n as u64, "");
            run_phash_postvalidation(&mut clusters, &hashes, &cfg);
            run_phash_oversized_split(&mut clusters, &hashes, &cfg);
            timings.push(StageTiming {
                name: "组内校验".into(),
                duration_ms: stage_start.elapsed().as_millis() as u64,
            });
        }
    }

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

// ---------------------------------------------------------------------------
// Shared post-validation helpers (used by phash and phash_ai modes)
// ---------------------------------------------------------------------------

fn run_phash_postvalidation(clusters: &mut Clusters, hashes: &[i64], cfg: &DedupConfig) {
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
            clusters.evict(m);
        }
    }
}

fn run_phash_oversized_split(clusters: &mut Clusters, hashes: &[i64], cfg: &DedupConfig) {
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
