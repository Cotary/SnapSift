use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub target_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFolder {
    pub id: String,
    pub project_id: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDetail {
    pub project: Project,
    pub source_folders: Vec<SourceFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub project_id: String,
    pub path: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_type: String,
    pub taken_at: Option<String>,
    pub phash: Option<i64>,
    pub md5: Option<String>,
    pub group_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub total: u64,
    pub scanned: u64,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub total_files: u64,
    pub images: u64,
    pub videos: u64,
    pub audios: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeProgress {
    pub total: u64,
    pub processed: u64,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizeResult {
    pub total: u64,
    pub success: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub files: Vec<FileRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupProgress {
    pub stage: String,
    pub current: u64,
    pub total: u64,
    pub current_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTiming {
    pub name: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupResult {
    pub total_target_images: u64,
    pub groups_found: u64,
    pub total_duplicates: u64,
    pub methods: Vec<String>,
    pub suspect_pairs_checked: u64,
    pub ai_confirmed: u64,
    pub timings: Vec<StageTiming>,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatus {
    pub available: bool,
    pub model_name: String,
    pub engine: String,
    pub message: String,
}
