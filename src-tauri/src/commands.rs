use tauri::State;

use crate::db::Database;
use crate::models::{
    AiStatus, DedupResult, DuplicateGroup, FileRecord, OrganizeResult, Project, ProjectDetail,
    ScanResult, SourceFolder,
};

#[tauri::command]
pub fn create_project(name: String, state: State<Database>) -> Result<Project, String> {
    state.create_project(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_projects(state: State<Database>) -> Result<Vec<Project>, String> {
    state.list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_project_detail(
    project_id: String,
    state: State<Database>,
) -> Result<ProjectDetail, String> {
    state
        .get_project_detail(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Project not found: {}", project_id))
}

#[tauri::command]
pub fn add_source_folders(
    project_id: String,
    paths: Vec<String>,
    state: State<Database>,
) -> Result<Vec<SourceFolder>, String> {
    state
        .add_source_folders(&project_id, &paths)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_source_folder(folder_id: String, state: State<Database>) -> Result<bool, String> {
    state
        .remove_source_folder(&folder_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_target_dir(
    project_id: String,
    path: String,
    state: State<Database>,
) -> Result<bool, String> {
    state
        .set_target_dir(&project_id, &path)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_project(project_id: String, state: State<Database>) -> Result<bool, String> {
    state
        .delete_project(&project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_scan(
    project_id: String,
    state: State<'_, Database>,
    window: tauri::Window,
) -> Result<ScanResult, String> {
    let folders = state
        .get_source_folders(&project_id)
        .map_err(|e| e.to_string())?;
    let folder_paths: Vec<String> = folders.into_iter().map(|f| f.path).collect();
    crate::scanner::scan_project(&state, &project_id, &folder_paths, &window)
}

#[tauri::command]
pub fn get_project_files(
    project_id: String,
    state: State<Database>,
) -> Result<Vec<FileRecord>, String> {
    state
        .get_project_files(&project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn organize_files(
    project_id: String,
    pattern: String,
    mode: String,
    file_types: Vec<String>,
    state: State<'_, Database>,
    window: tauri::Window,
) -> Result<OrganizeResult, String> {
    let project = state
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Project not found".to_string())?;
    let target_dir = project
        .target_dir
        .ok_or_else(|| "Target directory not set".to_string())?;

    // Step 1: Auto-rescan source folders so DB is up-to-date
    let folders = state
        .get_source_folders(&project_id)
        .map_err(|e| e.to_string())?;
    let folder_paths: Vec<String> = folders.into_iter().map(|f| f.path).collect();
    crate::scanner::scan_project(&state, &project_id, &folder_paths, &window)?;

    // Step 2: Clean up stale DB records for target files that no longer exist on disk
    state
        .cleanup_stale_target_files(&project_id, &target_dir)
        .map_err(|e| e.to_string())?;

    // Step 3: Fetch files and split into source vs target
    let all_files = state
        .get_project_files(&project_id)
        .map_err(|e| e.to_string())?;
    let normalized_target = target_dir.replace('\\', "/");
    let mut source_files = Vec::new();
    let mut target_files = Vec::new();
    for f in all_files {
        if f.path.replace('\\', "/").starts_with(&normalized_target) {
            target_files.push(f);
        } else {
            source_files.push(f);
        }
    }

    // Step 4: Execute organize
    let result = crate::organizer::organize_files(
        &source_files,
        &target_files,
        &target_dir,
        &pattern,
        &mode,
        &file_types,
        &window,
    )?;

    Ok(result)
}

#[tauri::command]
pub async fn scan_target(
    project_id: String,
    state: State<'_, Database>,
    window: tauri::Window,
) -> Result<ScanResult, String> {
    let project = state
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Project not found".to_string())?;
    let target_dir = project
        .target_dir
        .ok_or_else(|| "Target directory not set".to_string())?;
    crate::scanner::scan_folder(&state, &project_id, &target_dir, &window)
}

#[tauri::command]
pub async fn find_duplicates(
    project_id: String,
    phash_threshold: Option<u32>,
    cosine_threshold: Option<f32>,
    state: State<'_, Database>,
    window: tauri::Window,
) -> Result<DedupResult, String> {
    let project = state
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Project not found".to_string())?;
    let target_dir = project.target_dir.as_deref();

    let scan_start = std::time::Instant::now();
    if let Some(dir) = target_dir {
        state
            .cleanup_stale_target_files(&project_id, dir)
            .map_err(|e| e.to_string())?;
        crate::scanner::scan_folder(&state, &project_id, dir, &window)?;
    }
    let scan_ms = scan_start.elapsed().as_millis() as u64;

    let mut result =
        crate::dedup::find_duplicates(&state, &project_id, phash_threshold, cosine_threshold, target_dir, &window)?;

    result.timings.insert(
        0,
        crate::models::StageTiming {
            name: "扫描目标文件夹".into(),
            duration_ms: scan_ms,
        },
    );
    result.total_duration_ms += scan_ms;

    Ok(result)
}

#[tauri::command]
pub fn get_duplicate_groups(
    project_id: String,
    state: State<Database>,
) -> Result<Vec<DuplicateGroup>, String> {
    state
        .get_duplicate_groups(&project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_ai_status() -> AiStatus {
    let available = crate::embedder::is_available();
    let engine = crate::embedder::get_engine();
    if available {
        AiStatus {
            available: true,
            model_name: "MobileNet-v3-Small".to_string(),
            engine: crate::embedder::engine_name(engine).to_string(),
            message: "AI 向量检测已启用".to_string(),
        }
    } else {
        AiStatus {
            available: false,
            model_name: String::new(),
            engine: String::new(),
            message: "AI 模型未加载，仅使用 pHash".to_string(),
        }
    }
}

#[tauri::command]
pub fn set_ai_engine(engine: String) -> Result<String, String> {
    let e = match engine.as_str() {
        "ort" => crate::embedder::AiEngine::Ort,
        "tract" => crate::embedder::AiEngine::Tract,
        _ => return Err(format!("Unknown engine: {}", engine)),
    };
    crate::embedder::set_engine(e);
    Ok(crate::embedder::engine_name(e).to_string())
}

#[tauri::command]
pub async fn get_thumbnail(path: String, max_size: Option<u32>) -> Result<String, String> {
    let size = max_size.unwrap_or(300);
    tauri::async_runtime::spawn_blocking(move || {
        crate::thumbnail::generate_thumbnail(&path, size)
    })
    .await
    .map_err(|e| format!("Thumbnail task failed: {}", e))?
}

#[tauri::command]
pub fn delete_files(
    project_id: String,
    paths: Vec<String>,
    state: State<Database>,
) -> Result<u64, String> {
    let project = state
        .get_project(&project_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Project not found".to_string())?;
    let target_dir = project
        .target_dir
        .ok_or_else(|| "Target directory not set".to_string())?;
    state
        .delete_files_by_paths(&paths, &target_dir)
        .map_err(|e| e.to_string())
}
