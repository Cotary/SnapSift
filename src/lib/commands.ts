import { invoke } from "@tauri-apps/api/core";
import type {
  AiStatus,
  DedupResult,
  DuplicateGroup,
  FileRecord,
  OrganizeResult,
  Project,
  ProjectDetail,
  ScanResult,
  SourceFolder,
} from "./types";

export async function createProject(name: string): Promise<Project> {
  return invoke("create_project", { name });
}

export async function listProjects(): Promise<Project[]> {
  return invoke("list_projects");
}

export async function getProjectDetail(
  projectId: string
): Promise<ProjectDetail> {
  return invoke("get_project_detail", { projectId });
}

export async function addSourceFolders(
  projectId: string,
  paths: string[]
): Promise<SourceFolder[]> {
  return invoke("add_source_folders", { projectId, paths });
}

export async function removeSourceFolder(folderId: string): Promise<boolean> {
  return invoke("remove_source_folder", { folderId });
}

export async function setTargetDir(
  projectId: string,
  path: string
): Promise<boolean> {
  return invoke("set_target_dir", { projectId, path });
}

export async function deleteProject(projectId: string): Promise<boolean> {
  return invoke("delete_project", { projectId });
}

export async function startScan(projectId: string): Promise<ScanResult> {
  return invoke("start_scan", { projectId });
}

export async function getProjectFiles(
  projectId: string
): Promise<FileRecord[]> {
  return invoke("get_project_files", { projectId });
}

export async function organizeFiles(
  projectId: string,
  pattern: string,
  mode: string,
  fileTypes: string[]
): Promise<OrganizeResult> {
  return invoke("organize_files", { projectId, pattern, mode, fileTypes });
}

export async function scanTarget(projectId: string): Promise<ScanResult> {
  return invoke("scan_target", { projectId });
}

export async function findDuplicates(
  projectId: string,
  phashThreshold?: number,
  cosineThreshold?: number
): Promise<DedupResult> {
  return invoke("find_duplicates", { projectId, phashThreshold, cosineThreshold });
}

export async function getDuplicateGroups(
  projectId: string
): Promise<DuplicateGroup[]> {
  return invoke("get_duplicate_groups", { projectId });
}

export async function getAiStatus(): Promise<AiStatus> {
  return invoke("get_ai_status");
}

export async function setAiEngine(engine: "tract" | "ort"): Promise<string> {
  return invoke("set_ai_engine", { engine });
}

const thumbnailQueue = (() => {
  const MAX_CONCURRENT = 6;
  let running = 0;
  const queue: Array<() => void> = [];
  function next() {
    if (queue.length > 0 && running < MAX_CONCURRENT) {
      running++;
      const resolve = queue.shift()!;
      resolve();
    }
  }
  function acquire(): Promise<void> {
    return new Promise<void>((resolve) => {
      queue.push(resolve);
      next();
    });
  }
  function release() {
    running--;
    next();
  }
  return { acquire, release };
})();

export async function getThumbnail(
  path: string,
  maxSize?: number
): Promise<string> {
  await thumbnailQueue.acquire();
  try {
    return await invoke<string>("get_thumbnail", { path, maxSize });
  } finally {
    thumbnailQueue.release();
  }
}

export async function deleteFiles(
  projectId: string,
  paths: string[]
): Promise<number> {
  return invoke("delete_files", { projectId, paths });
}
