export interface Project {
  id: string;
  name: string;
  target_dir: string | null;
  created_at: string;
  updated_at: string;
}

export interface SourceFolder {
  id: string;
  project_id: string;
  path: string;
  created_at: string;
}

export interface ProjectDetail {
  project: Project;
  source_folders: SourceFolder[];
}

export interface FileRecord {
  id: string;
  project_id: string;
  path: string;
  file_name: string;
  file_size: number;
  file_type: string;
  taken_at: string | null;
  phash: number | null;
  md5: string | null;
  group_id: string | null;
  created_at: string;
}

export interface ScanProgress {
  total: number;
  scanned: number;
  current_file: string;
}

export interface ScanResult {
  total_files: number;
  images: number;
  videos: number;
  audios: number;
}

export interface OrganizeProgress {
  total: number;
  processed: number;
  current_file: string;
}

export interface OrganizeResult {
  total: number;
  success: number;
  skipped: number;
}

export interface DuplicateGroup {
  group_id: string;
  files: FileRecord[];
}

export interface DedupProgress {
  stage: string;
  current: number;
  total: number;
  current_file: string;
}

export interface StageTiming {
  name: string;
  duration_ms: number;
}

export interface DedupResult {
  total_target_images: number;
  groups_found: number;
  total_duplicates: number;
  methods: string[];
  suspect_pairs_checked: number;
  ai_confirmed: number;
  timings: StageTiming[];
  total_duration_ms: number;
}

export interface AiStatus {
  available: boolean;
  model_name: string;
  engine: string;
  message: string;
}
