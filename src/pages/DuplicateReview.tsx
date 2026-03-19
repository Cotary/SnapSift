import { useState, useRef, useCallback, useEffect, useMemo } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useQuery, useMutation, useQueryClient, useIsFetching } from "@tanstack/react-query";
import { useVirtualizer } from "@tanstack/react-virtual";
import { listen } from "@tauri-apps/api/event";
import {
  findDuplicates,
  getDuplicateGroups,
  deleteFiles,
  getProjectDetail,
  getAiStatus,
  setAiEngine,
} from "@/lib/commands";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import DuplicateGroupCard from "@/components/DuplicateGroupCard";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DuplicateGroup, DedupResult, DedupProgress } from "@/lib/types";
import {
  ArrowLeft,
  Loader2,
  Search,
  Trash2,
  ImageIcon,
  ShieldCheck,
  ShieldAlert,
  Brain,
  Hash,
  Clock,
  Timer,
  SlidersHorizontal,
} from "lucide-react";

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const min = Math.floor(ms / 60000);
  const sec = ((ms % 60000) / 1000).toFixed(0);
  return `${min}m${sec}s`;
}

export default function DuplicateReview() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const parentRef = useRef<HTMLDivElement>(null);

  // selections[fileId] = true (keep) | false (delete)
  const [selections, setSelections] = useState<Record<string, boolean>>({});
  const [confirmOpen, setConfirmOpen] = useState(false);

  const { data: projectDetail } = useQuery({
    queryKey: ["project", id],
    queryFn: () => getProjectDetail(id!),
    enabled: !!id,
  });

  const targetDir = projectDetail?.project?.target_dir ?? "";
  const hasTargetDir = !!targetDir;

  const { data: aiStatus, refetch: refetchAiStatus } = useQuery({
    queryKey: ["ai-status"],
    queryFn: () => getAiStatus(),
  });

  const engineMutation = useMutation({
    mutationFn: (engine: "tract" | "ort") => setAiEngine(engine),
    onSuccess: () => {
      refetchAiStatus();
    },
  });

  const currentEngine = aiStatus?.engine?.includes("ort") ? "ort" : "tract";

  const [lastDedupResult, setLastDedupResult] = useState<DedupResult | null>(null);
  const [dedupProgress, setDedupProgress] = useState<DedupProgress | null>(null);

  const [analysisMode, setAnalysisMode] = useState<"phash" | "phash_ai" | "ai">("phash_ai");
  const [phashThreshold, setPhashThreshold] = useState(8);
  const [cosineThreshold, setCosineThreshold] = useState(93);

  const thumbnailFetchingCount = useIsFetching({ queryKey: ["thumbnail"] });
  const thumbTimerRef = useRef<number>(0);
  const [thumbnailLoadMs, setThumbnailLoadMs] = useState<number | null>(null);

  useEffect(() => {
    const unlistenDedup = listen<DedupProgress>("dedup-progress", (event) => {
      setDedupProgress(event.payload);
    });
    const unlistenScan = listen<{ total: number; scanned: number; current_file: string }>(
      "scan-progress",
      (event) => {
        const { total, scanned, current_file } = event.payload;
        setDedupProgress({
          stage: "扫描目标文件夹",
          current: scanned,
          total,
          current_file,
        });
      }
    );
    return () => {
      unlistenDedup.then((fn) => fn());
      unlistenScan.then((fn) => fn());
    };
  }, []);

  const findMutation = useMutation({
    mutationFn: () => findDuplicates(id!, analysisMode, phashThreshold, cosineThreshold / 100),
    onSuccess: (result) => {
      setLastDedupResult(result);
      setDedupProgress(null);
      setThumbnailLoadMs(null);
      thumbTimerRef.current = Date.now();
      queryClient.invalidateQueries({ queryKey: ["duplicate-groups", id] });
    },
    onError: () => {
      setDedupProgress(null);
    },
  });

  const {
    data: groups = [],
    isLoading: groupsLoading,
    refetch: refetchGroups,
  } = useQuery({
    queryKey: ["duplicate-groups", id],
    queryFn: () => getDuplicateGroups(id!),
    enabled: !!id,
  });

  useEffect(() => {
    if (
      thumbnailFetchingCount === 0 &&
      thumbTimerRef.current > 0 &&
      thumbnailLoadMs === null &&
      groups.length > 0
    ) {
      setThumbnailLoadMs(Date.now() - thumbTimerRef.current);
      thumbTimerRef.current = 0;
    }
  }, [thumbnailFetchingCount, thumbnailLoadMs, groups.length]);

  const initSelections = useCallback(
    (groups: DuplicateGroup[]) => {
      const newSelections: Record<string, boolean> = {};
      for (const group of groups) {
        group.files.forEach((f, i) => {
          if (!(f.id in selections)) {
            newSelections[f.id] = i === 0;
          } else {
            newSelections[f.id] = selections[f.id];
          }
        });
      }
      setSelections((prev) => ({ ...prev, ...newSelections }));
    },
    [selections]
  );

  // Initialize selections when groups load
  const prevGroupsRef = useRef<string>("");
  const groupsKey = groups.map((g) => g.group_id).join(",");
  if (groupsKey !== prevGroupsRef.current && groups.length > 0) {
    prevGroupsRef.current = groupsKey;
    initSelections(groups);
  }

  const deleteMutation = useMutation({
    mutationFn: (paths: string[]) => deleteFiles(id!, paths),
    onSuccess: () => {
      setConfirmOpen(false);
      setSelections({});
      refetchGroups();
    },
  });

  // Left click: "keep only this" — mark clicked as keep, others as delete
  const handleSelect = (groupId: string, fileId: string) => {
    const group = groups.find((g) => g.group_id === groupId);
    if (!group) return;
    setSelections((prev) => {
      const next = { ...prev };
      for (const f of group.files) {
        next[f.id] = f.id === fileId;
      }
      return next;
    });
  };

  // Right click: toggle single file keep/delete
  const handleToggle = (fileId: string) => {
    setSelections((prev) => ({
      ...prev,
      [fileId]: !prev[fileId],
    }));
  };

  // Keep all files in a group
  const handleKeepAll = useCallback(
    (groupId: string) => {
      const group = groups.find((g) => g.group_id === groupId);
      if (!group) return;
      setSelections((prev) => {
        const next = { ...prev };
        for (const f of group.files) next[f.id] = true;
        return next;
      });
    },
    [groups]
  );

  // Mark all files in a group for deletion
  const handleDeleteAll = useCallback(
    (groupId: string) => {
      const group = groups.find((g) => g.group_id === groupId);
      if (!group) return;
      setSelections((prev) => {
        const next = { ...prev };
        for (const f of group.files) next[f.id] = false;
        return next;
      });
    },
    [groups]
  );

  // Keep all files from groupIndex onward
  const handleSkipBelow = useCallback(
    (groupIndex: number) => {
      setSelections((prev) => {
        const next = { ...prev };
        for (let i = groupIndex; i < groups.length; i++) {
          for (const f of groups[i].files) next[f.id] = true;
        }
        return next;
      });
    },
    [groups]
  );

  const filesToDelete = useMemo(
    () => groups.flatMap((g) => g.files).filter((f) => selections[f.id] === false),
    [groups, selections]
  );

  const totalDeleteSize = useMemo(
    () => filesToDelete.reduce((sum, f) => sum + f.file_size, 0),
    [filesToDelete]
  );

  const virtualizer = useVirtualizer({
    count: groups.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 320,
    overscan: 3,
  });

  return (
    <div className="flex flex-col h-screen">
      <header className="flex items-center gap-4 px-6 py-4 border-b shrink-0">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate(`/project/${id}`)}
          className="shrink-0"
        >
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <div className="flex-1 min-w-0">
          <h1 className="text-lg font-bold">相似图片筛选</h1>
          <p className="text-sm text-muted-foreground">
            左键点击: 仅保留该图 &middot; 右键点击: 切换单张状态
          </p>
          <div className="flex items-center gap-2 mt-1.5 flex-wrap">
            <Badge variant="secondary" className="gap-1 text-xs">
              <Hash className="h-3 w-3" />
              pHash
            </Badge>
            {aiStatus?.available ? (
              <>
                <Badge variant="secondary" className="gap-1 text-xs bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200">
                  <Brain className="h-3 w-3" />
                  {aiStatus.model_name}
                  <ShieldCheck className="h-3 w-3" />
                </Badge>
                <Select
                  value={currentEngine}
                  onValueChange={(val) => { if (val) engineMutation.mutate(val as "tract" | "ort"); }}
                  disabled={findMutation.isPending}
                >
                  <SelectTrigger className="h-6 w-auto gap-1.5 text-xs px-2 py-0">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="tract">tract (Pure Rust)</SelectItem>
                    <SelectItem value="ort">ort (ONNX Runtime)</SelectItem>
                  </SelectContent>
                </Select>
              </>
            ) : (
              <Badge variant="outline" className="gap-1 text-xs text-amber-600">
                <ShieldAlert className="h-3 w-3" />
                AI 未启用
              </Badge>
            )}
            {lastDedupResult && (
              <>
                <Badge variant="outline" className="gap-1 text-xs">
                  图片 {lastDedupResult.total_target_images}
                </Badge>
                <Badge variant="outline" className="gap-1 text-xs">
                  {lastDedupResult.groups_found} 组 / {lastDedupResult.total_duplicates} 张疑似重复
                </Badge>
                {lastDedupResult.suspect_pairs_checked > 0 && (
                  <Badge variant="outline" className="gap-1 text-xs">
                    AI确认 {lastDedupResult.ai_confirmed}
                  </Badge>
                )}
                <Badge variant="secondary" className="gap-1 text-xs">
                  <Timer className="h-3 w-3" />
                  总耗时 {formatDuration(lastDedupResult.total_duration_ms)}
                </Badge>
              </>
            )}
          </div>
        </div>
        <div className="flex items-center gap-4 shrink-0">
          <div className="flex flex-col gap-2 text-xs">
            <div className="flex items-center gap-2">
              <SlidersHorizontal className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
              <Select
                value={analysisMode}
                onValueChange={(val) => { if (val) setAnalysisMode(val as "phash" | "phash_ai" | "ai"); }}
                disabled={findMutation.isPending}
              >
                <SelectTrigger className="h-6 w-auto gap-1.5 text-xs px-2 py-0">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="phash">pHash Only</SelectItem>
                  <SelectItem value="phash_ai">pHash + AI</SelectItem>
                  {aiStatus?.available && <SelectItem value="ai">Pure AI</SelectItem>}
                </SelectContent>
              </Select>
            </div>
            {analysisMode !== "ai" && (
              <div className="flex items-center gap-2">
                <Hash className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                <label className="w-20 text-muted-foreground shrink-0">pHash ≤ {phashThreshold}</label>
                <input
                  type="range"
                  min={2}
                  max={16}
                  step={1}
                  value={phashThreshold}
                  onChange={(e) => setPhashThreshold(Number(e.target.value))}
                  disabled={findMutation.isPending}
                  className="w-28 h-1.5 accent-primary cursor-pointer"
                />
                <span className="text-muted-foreground w-14 text-right">{phashThreshold <= 5 ? "严格" : phashThreshold <= 8 ? "推荐" : phashThreshold <= 12 ? "宽松" : "极宽松"}</span>
              </div>
            )}
            {analysisMode !== "phash" && aiStatus?.available && (
              <div className="flex items-center gap-2">
                <Brain className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                <label className="w-20 text-muted-foreground shrink-0">AI ≥ {cosineThreshold}%</label>
                <input
                  type="range"
                  min={80}
                  max={99}
                  step={1}
                  value={cosineThreshold}
                  onChange={(e) => setCosineThreshold(Number(e.target.value))}
                  disabled={findMutation.isPending}
                  className="w-28 h-1.5 accent-primary cursor-pointer"
                />
                <span className="text-muted-foreground w-14 text-right">{cosineThreshold >= 96 ? "严格" : cosineThreshold >= 93 ? "推荐" : cosineThreshold >= 88 ? "宽松" : "极宽松"}</span>
              </div>
            )}
          </div>
          <Button
            onClick={() => findMutation.mutate()}
            disabled={findMutation.isPending || !hasTargetDir}
            variant="outline"
            className="gap-2"
          >
            {findMutation.isPending ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <Search className="h-4 w-4" />
            )}
            {findMutation.isPending ? "分析中..." : "重新分析"}
          </Button>
        </div>
      </header>

      {findMutation.isPending && dedupProgress && (
        <div className="px-6 py-3 border-b bg-muted/30 shrink-0 space-y-2">
          <div className="flex items-center justify-between text-sm">
            <span className="font-medium">{dedupProgress.stage}</span>
            <span className="text-muted-foreground">
              {dedupProgress.current} / {dedupProgress.total}
            </span>
          </div>
          <Progress
            value={
              dedupProgress.total > 0
                ? (dedupProgress.current / dedupProgress.total) * 100
                : 0
            }
            className="h-2"
          />
          {dedupProgress.current_file && (
            <p className="text-xs text-muted-foreground truncate" title={dedupProgress.current_file}>
              {dedupProgress.current_file}
            </p>
          )}
        </div>
      )}

      {lastDedupResult && lastDedupResult.timings.length > 0 && !findMutation.isPending && (
        <div className="px-6 py-2 border-b bg-muted/20 shrink-0">
          <div className="flex items-center gap-4 flex-wrap text-xs text-muted-foreground">
            <span className="flex items-center gap-1 font-medium text-foreground">
              <Clock className="h-3.5 w-3.5" />
              各阶段耗时
            </span>
            {lastDedupResult.timings.map((t) => (
              <span key={t.name} className="flex items-center gap-1">
                {t.name}: <span className="font-mono text-foreground">{formatDuration(t.duration_ms)}</span>
              </span>
            ))}
            {thumbnailLoadMs !== null && (
              <span className="flex items-center gap-1">
                缩略图加载: <span className="font-mono text-foreground">{formatDuration(thumbnailLoadMs)}</span>
              </span>
            )}
          </div>
        </div>
      )}

      {!hasTargetDir ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-4 text-muted-foreground">
          <ImageIcon className="h-16 w-16 opacity-30" />
          <div className="text-center">
            <p className="text-lg font-medium">请先设置目标文件夹</p>
            <p className="text-sm mt-1">
              回到项目页面设置目标文件夹，执行文件整理后再来去重
            </p>
          </div>
          <Button variant="outline" onClick={() => navigate(`/project/${id}`)}>
            返回项目
          </Button>
        </div>
      ) : groupsLoading ? (
        <div className="flex-1 flex items-center justify-center">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      ) : groups.length === 0 ? (
        <div className="flex-1 flex flex-col items-center justify-center gap-4 text-muted-foreground">
          <ImageIcon className="h-16 w-16 opacity-30" />
          <div className="text-center">
            <p className="text-lg font-medium">未发现相似图片</p>
            <p className="text-sm mt-1">
              请先在项目页面扫描文件，然后点击"重新分析"
            </p>
          </div>
        </div>
      ) : (
        <div ref={parentRef} className="flex-1 overflow-auto">
          <div
            className="relative w-full"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const group = groups[virtualRow.index];
              return (
                <div
                  key={group.group_id}
                  data-index={virtualRow.index}
                  ref={virtualizer.measureElement}
                  className="absolute top-0 left-0 w-full px-6 py-2"
                  style={{
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                >
                  <DuplicateGroupCard
                    groupId={group.group_id}
                    groupIndex={virtualRow.index}
                    files={group.files}
                    targetDir={targetDir}
                    groupSelections={Object.fromEntries(
                      group.files.map((f) => [f.id, selections[f.id] ?? true])
                    )}
                    onSelect={(fileId) => handleSelect(group.group_id, fileId)}
                    onToggle={handleToggle}
                    onKeepAll={() => handleKeepAll(group.group_id)}
                    onDeleteAll={() => handleDeleteAll(group.group_id)}
                    onSkipBelow={() => handleSkipBelow(virtualRow.index)}
                  />
                </div>
              );
            })}
          </div>
        </div>
      )}

      {groups.length > 0 && (
        <footer className="flex items-center justify-between px-6 py-3 border-t bg-background shrink-0">
          <div className="text-sm text-muted-foreground">
            {groups.length} 组相似图片 &middot; 待删除{" "}
            <span className="font-medium text-foreground">{filesToDelete.length}</span>{" "}
            张 &middot; 预计释放{" "}
            <span className="font-medium text-foreground">
              {formatSize(totalDeleteSize)}
            </span>
          </div>
          <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
            <DialogTrigger
              render={
                <Button
                  variant="destructive"
                  className="gap-2"
                  disabled={filesToDelete.length === 0}
                />
              }
            >
              <Trash2 className="h-4 w-4" />
              执行删除 ({filesToDelete.length})
            </DialogTrigger>
            <DialogContent className="max-w-sm">
              <DialogHeader>
                <DialogTitle>确认删除</DialogTitle>
                <DialogDescription>
                  即将永久删除 <span className="font-semibold text-foreground">{filesToDelete.length}</span> 个文件，
                  释放 <span className="font-semibold text-foreground">{formatSize(totalDeleteSize)}</span> 空间，此操作不可撤销。
                </DialogDescription>
              </DialogHeader>
              <DialogFooter>
                <Button variant="outline" onClick={() => setConfirmOpen(false)}>
                  取消
                </Button>
                <Button
                  variant="destructive"
                  onClick={() =>
                    deleteMutation.mutate(filesToDelete.map((f) => f.path))
                  }
                  disabled={deleteMutation.isPending}
                  className="gap-2"
                >
                  {deleteMutation.isPending ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <Trash2 className="h-4 w-4" />
                  )}
                  确认删除
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </footer>
      )}
    </div>
  );
}
