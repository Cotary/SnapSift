import { useState, useMemo, useRef, useCallback, memo } from "react";
import { createPortal } from "react-dom";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { FileRecord } from "@/lib/types";
import ThumbnailImage from "./ThumbnailImage";
import { Check, Trash2, ChevronDown, ChevronUp, AlertTriangle, FolderOpen, Calendar, HardDrive, FileImage, Loader2, CheckCheck, XCircle, ChevronsDown } from "lucide-react";

interface DuplicateGroupCardProps {
  groupId: string;
  groupIndex: number;
  files: FileRecord[];
  targetDir: string;
  groupSelections: Record<string, boolean>;
  onSelect: (fileId: string) => void;
  onToggle: (fileId: string) => void;
  onKeepAll: () => void;
  onDeleteAll: () => void;
  onSkipBelow: () => void;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const FOLDER_COLORS = [
  { bg: "bg-blue-100 dark:bg-blue-900/40", text: "text-blue-700 dark:text-blue-300", ring: "ring-blue-400" },
  { bg: "bg-amber-100 dark:bg-amber-900/40", text: "text-amber-700 dark:text-amber-300", ring: "ring-amber-400" },
  { bg: "bg-emerald-100 dark:bg-emerald-900/40", text: "text-emerald-700 dark:text-emerald-300", ring: "ring-emerald-400" },
  { bg: "bg-purple-100 dark:bg-purple-900/40", text: "text-purple-700 dark:text-purple-300", ring: "ring-purple-400" },
  { bg: "bg-rose-100 dark:bg-rose-900/40", text: "text-rose-700 dark:text-rose-300", ring: "ring-rose-400" },
  { bg: "bg-cyan-100 dark:bg-cyan-900/40", text: "text-cyan-700 dark:text-cyan-300", ring: "ring-cyan-400" },
];

function getRelativeFolder(filePath: string, targetDir: string): string {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/$/, "");
  const normPath = norm(filePath);
  const normTarget = norm(targetDir);
  const lastSlash = normPath.lastIndexOf("/");
  const dirPath = lastSlash >= 0 ? normPath.substring(0, lastSlash) : "";
  if (dirPath.startsWith(normTarget)) {
    const relative = dirPath.substring(normTarget.length).replace(/^\//, "");
    return relative || "/";
  }
  return dirPath;
}

function HoverPreview({ file, pos }: { file: FileRecord; pos: { x: number; y: number } }) {
  const [loaded, setLoaded] = useState(false);
  const originalSrc = convertFileSrc(file.path);

  return (
    <div
      className="fixed rounded-lg shadow-xl border bg-popover overflow-hidden animate-in fade-in-0 zoom-in-95 duration-150"
      style={{
        zIndex: 9999,
        left: Math.min(pos.x, window.innerWidth - 520),
        top: Math.max(8, Math.min(pos.y, window.innerHeight - 560)),
        pointerEvents: "none",
      }}
    >
      <div className="relative w-[500px] h-[500px] bg-muted/30">
        {!loaded && (
          <div className="absolute inset-0 flex items-center justify-center">
            <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
          </div>
        )}
        <img
          src={originalSrc}
          alt={file.file_name}
          className="w-full h-full object-contain"
          onLoad={() => setLoaded(true)}
          draggable={false}
        />
      </div>
      <div className="p-3 space-y-1.5 text-xs">
        <p className="font-medium truncate" title={file.file_name}>
          {file.file_name}
        </p>
        <div className="flex items-center gap-3 text-muted-foreground">
          <span className="flex items-center gap-1">
            <HardDrive className="h-3 w-3" />
            {formatSize(file.file_size)}
          </span>
          {file.taken_at && (
            <span className="flex items-center gap-1">
              <Calendar className="h-3 w-3" />
              {file.taken_at.split("T")[0]}
            </span>
          )}
          <span className="flex items-center gap-1">
            <FileImage className="h-3 w-3" />
            {file.file_name.split(".").pop()?.toUpperCase()}
          </span>
          <span>{file.file_size > 1024 * 1024 ? `${(file.file_size / (1024 * 1024)).toFixed(1)} MB` : `${(file.file_size / 1024).toFixed(1)} KB`}</span>
        </div>
      </div>
    </div>
  );
}

function DuplicateGroupCard({
  groupIndex,
  files,
  targetDir,
  groupSelections: selections,
  onSelect,
  onToggle,
  onKeepAll,
  onDeleteAll,
  onSkipBelow,
}: DuplicateGroupCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [hoverFile, setHoverFile] = useState<FileRecord | null>(null);
  const [hoverPos, setHoverPos] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleMouseEnter = useCallback((file: FileRecord, e: React.MouseEvent) => {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    hoverTimerRef.current = setTimeout(() => {
      setHoverFile(file);
      setHoverPos({ x: rect.right + 8, y: rect.top });
    }, 350);
  }, []);

  const handleMouseLeave = useCallback(() => {
    if (hoverTimerRef.current) {
      clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    setHoverFile(null);
  }, []);

  const sortedFiles = useMemo(
    () => [...files].sort((a, b) => b.file_size - a.file_size),
    [files]
  );

  const { folders, folderColorMap, hasMultipleFolders } = useMemo(() => {
    const folderSet = new Map<string, FileRecord[]>();
    for (const f of sortedFiles) {
      const rel = getRelativeFolder(f.path, targetDir);
      const list = folderSet.get(rel) || [];
      list.push(f);
      folderSet.set(rel, list);
    }
    const colorMap = new Map<string, (typeof FOLDER_COLORS)[0]>();
    let idx = 0;
    for (const key of folderSet.keys()) {
      colorMap.set(key, FOLDER_COLORS[idx % FOLDER_COLORS.length]);
      idx++;
    }
    return {
      folders: folderSet,
      folderColorMap: colorMap,
      hasMultipleFolders: folderSet.size > 1,
    };
  }, [sortedFiles, targetDir]);

  const allKept = sortedFiles.every((f) => selections[f.id] !== false);
  const allDeleted = sortedFiles.every((f) => selections[f.id] === false);

  return (
    <div className="rounded-lg border bg-card p-4 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-muted-foreground">第 {groupIndex + 1} 组</span>
          {hasMultipleFolders && (
            <div className="flex items-center gap-1 text-xs text-amber-600 dark:text-amber-400">
              <AlertTriangle className="h-3.5 w-3.5 shrink-0" />
              来自 {folders.size} 个不同文件夹
            </div>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          <button
            onClick={onKeepAll}
            disabled={allKept}
            className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded border hover:bg-accent disabled:opacity-40 transition-colors"
            title="全部保留"
          >
            <CheckCheck className="h-3 w-3 text-green-600" />
            全选
          </button>
          <button
            onClick={onDeleteAll}
            disabled={allDeleted}
            className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded border hover:bg-accent disabled:opacity-40 transition-colors"
            title="全部标记删除"
          >
            <XCircle className="h-3 w-3 text-red-500" />
            全不选
          </button>
          <button
            onClick={onSkipBelow}
            className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded border hover:bg-accent transition-colors text-muted-foreground hover:text-foreground"
            title="此组及以下全部保留（不删除）"
          >
            <ChevronsDown className="h-3 w-3" />
            以下都保留
          </button>
        </div>
      </div>

      <div className="flex gap-3 overflow-x-auto pb-2">
        {sortedFiles.map((file, fileIndex) => {
          const isKept = selections[file.id] !== false;
          const relFolder = getRelativeFolder(file.path, targetDir);
          const color = folderColorMap.get(relFolder) || FOLDER_COLORS[0];

          return (
            <div
              key={file.id}
              className="relative flex-shrink-0 cursor-pointer group"
              onClick={(e) => {
                e.preventDefault();
                onSelect(file.id);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                onToggle(file.id);
              }}
              onMouseEnter={(e) => handleMouseEnter(file, e)}
              onMouseLeave={handleMouseLeave}
            >
              <div
                className={`relative rounded-lg overflow-hidden transition-all ${
                  isKept
                    ? "ring-2 ring-green-500 ring-offset-2"
                    : "opacity-50 ring-2 ring-red-400 ring-offset-2"
                }`}
              >
                <ThumbnailImage
                  path={file.path}
                  className="w-40 h-40"
                  alt={file.file_name}
                />

                {isKept ? (
                  <div className="absolute top-1.5 right-1.5 bg-green-500 text-white rounded-full p-0.5">
                    <Check className="h-3.5 w-3.5" />
                  </div>
                ) : (
                  <div className="absolute top-1.5 right-1.5 bg-red-500 text-white rounded-full p-0.5">
                    <Trash2 className="h-3.5 w-3.5" />
                  </div>
                )}

                {fileIndex === 0 && (
                  <div className="absolute bottom-1.5 left-1.5 bg-green-600/90 text-white text-[10px] px-1.5 py-0.5 rounded">
                    推荐保留
                  </div>
                )}
              </div>

              <div className="mt-1.5 space-y-0.5 w-40">
                <p className="text-xs truncate" title={file.file_name}>
                  {file.file_name}
                </p>
                <p className="text-xs text-muted-foreground">
                  {formatSize(file.file_size)}
                </p>
                {hasMultipleFolders && (
                  <div
                    className={`text-[10px] px-1.5 py-0.5 rounded truncate ${color.bg} ${color.text}`}
                    title={relFolder}
                  >
                    {relFolder}
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>

      {hoverFile && createPortal(
        <HoverPreview file={hoverFile} pos={hoverPos} />,
        document.body
      )}

      {hasMultipleFolders && (
        <>
          <button
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors w-full"
          >
            {expanded ? (
              <ChevronUp className="h-3.5 w-3.5" />
            ) : (
              <ChevronDown className="h-3.5 w-3.5" />
            )}
            {expanded ? "收起详情" : "展开文件夹详情"}
          </button>

          {expanded && (
            <div className="space-y-3 pt-1">
              {Array.from(folders.entries()).map(([folder, folderFiles]) => {
                const color = folderColorMap.get(folder) || FOLDER_COLORS[0];
                return (
                  <div key={folder} className={`rounded-md border p-3 ${color.bg}`}>
                    <div className={`flex items-center gap-1.5 text-xs font-medium mb-2 ${color.text}`}>
                      <FolderOpen className="h-3.5 w-3.5 shrink-0" />
                      <span className="truncate" title={folder}>{folder}</span>
                      <span className="shrink-0">({folderFiles.length})</span>
                    </div>
                    <div className="space-y-1.5">
                      {folderFiles.map((f) => {
                        const isKept = selections[f.id] !== false;
                        return (
                          <div
                            key={f.id}
                            className={`flex items-center gap-2 text-xs rounded px-2 py-1 ${
                              isKept ? "bg-white/60 dark:bg-white/10" : "bg-red-50/60 dark:bg-red-900/20 line-through opacity-60"
                            }`}
                          >
                            {isKept ? (
                              <Check className="h-3 w-3 text-green-600 shrink-0" />
                            ) : (
                              <Trash2 className="h-3 w-3 text-red-500 shrink-0" />
                            )}
                            <span className="truncate flex-1" title={f.file_name}>{f.file_name}</span>
                            <span className="text-muted-foreground shrink-0">{formatSize(f.file_size)}</span>
                            {f.taken_at && (
                              <span className="text-muted-foreground shrink-0">{f.taken_at.split("T")[0]}</span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default memo(DuplicateGroupCard, (prev, next) => {
  if (prev.groupId !== next.groupId) return false;
  if (prev.groupIndex !== next.groupIndex) return false;
  if (prev.files !== next.files) return false;
  if (prev.targetDir !== next.targetDir) return false;
  const prevSel = prev.groupSelections;
  const nextSel = next.groupSelections;
  const keys = Object.keys(nextSel);
  if (keys.length !== Object.keys(prevSel).length) return false;
  for (const k of keys) {
    if (prevSel[k] !== nextSel[k]) return false;
  }
  return true;
});
