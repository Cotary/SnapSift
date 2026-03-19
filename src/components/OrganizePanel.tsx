import { useEffect, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { organizeFiles } from "@/lib/commands";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Checkbox } from "@/components/ui/checkbox";
import { Progress } from "@/components/ui/progress";
import { Badge } from "@/components/ui/badge";
import type { OrganizeProgress, OrganizeResult } from "@/lib/types";
import { CalendarDays, Copy, FolderInput, Loader2, ImageIcon, Film, Music } from "lucide-react";

const PATTERNS = [
  { value: "YYYY/MM/DD", label: "YYYY/MM/DD", example: "2024/05/20/IMG_001.jpg" },
  { value: "YYYY/MM", label: "YYYY/MM", example: "2024/05/IMG_001.jpg" },
  { value: "YYYY_MM", label: "YYYY_MM", example: "2024_05/IMG_001.jpg" },
  { value: "YYYY-MM/DD", label: "YYYY-MM/DD", example: "2024-05/20/IMG_001.jpg" },
  { value: "YYYY_MM/DD", label: "YYYY_MM/DD", example: "2024_05/20/IMG_001.jpg" },
  { value: "YYYY/YYYY-MM-DD", label: "YYYY/YYYY-MM-DD", example: "2024/2024-05-20/IMG_001.jpg" },
  { value: "YYYY", label: "YYYY", example: "2024/IMG_001.jpg" },
];

interface OrganizePanelProps {
  projectId: string;
  hasTargetDir: boolean;
  hasFiles: boolean;
}

export default function OrganizePanel({
  projectId,
  hasTargetDir,
  hasFiles,
}: OrganizePanelProps) {
  const [pattern, setPattern] = useState(PATTERNS[0].value);
  const [mode, setMode] = useState<"copy" | "move">("copy");
  const [fileTypes, setFileTypes] = useState<string[]>(["image", "video", "audio"]);
  const [progress, setProgress] = useState<OrganizeProgress | null>(null);
  const [result, setResult] = useState<OrganizeResult | null>(null);

  const toggleFileType = (type: string) => {
    setFileTypes((prev) =>
      prev.includes(type) ? prev.filter((t) => t !== type) : [...prev, type]
    );
  };

  const selectedPattern = PATTERNS.find((p) => p.value === pattern);

  const organizeMutation = useMutation({
    mutationFn: () => organizeFiles(projectId, pattern, mode, fileTypes),
    onSuccess: (data) => {
      setResult(data);
      setProgress(null);
    },
    onError: () => {
      setProgress(null);
    },
  });

  useEffect(() => {
    if (!organizeMutation.isPending) return;

    const unlisten = listen<OrganizeProgress>("organize-progress", (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [organizeMutation.isPending]);

  const percent =
    progress && progress.total > 0
      ? Math.round((progress.processed / progress.total) * 100)
      : 0;

  const disabled = !hasTargetDir || !hasFiles || fileTypes.length === 0 || organizeMutation.isPending;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2 text-base font-medium">
        <CalendarDays className="h-5 w-5" />
        按日期整理
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div className="space-y-2">
          <label className="text-sm font-medium">日期模板</label>
          <Select value={pattern} onValueChange={(v) => { if (v !== null) setPattern(v); }}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PATTERNS.map((p) => (
                <SelectItem key={p.value} value={p.value}>
                  {p.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {selectedPattern && (
            <p className="text-xs text-muted-foreground">
              预览: {selectedPattern.example}
            </p>
          )}
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium">操作模式</label>
          <div className="flex gap-2">
            <Button
              variant={mode === "copy" ? "default" : "outline"}
              size="sm"
              onClick={() => setMode("copy")}
              className="gap-1.5 flex-1"
            >
              <Copy className="h-3.5 w-3.5" />
              复制
            </Button>
            <Button
              variant={mode === "move" ? "default" : "outline"}
              size="sm"
              onClick={() => setMode("move")}
              className="gap-1.5 flex-1"
            >
              <FolderInput className="h-3.5 w-3.5" />
              移动
            </Button>
          </div>
        </div>
      </div>

      <div className="space-y-2">
        <label className="text-sm font-medium">文件类型</label>
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <Checkbox
              checked={fileTypes.includes("image")}
              onCheckedChange={() => toggleFileType("image")}
            />
            <ImageIcon className="h-3.5 w-3.5" />
            图片
          </label>
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <Checkbox
              checked={fileTypes.includes("video")}
              onCheckedChange={() => toggleFileType("video")}
            />
            <Film className="h-3.5 w-3.5" />
            视频
          </label>
          <label className="flex items-center gap-1.5 text-sm cursor-pointer">
            <Checkbox
              checked={fileTypes.includes("audio")}
              onCheckedChange={() => toggleFileType("audio")}
            />
            <Music className="h-3.5 w-3.5" />
            音频
          </label>
        </div>
      </div>

      {!hasTargetDir && (
        <p className="text-sm text-amber-600">请先设置目标文件夹</p>
      )}
      {!hasFiles && hasTargetDir && (
        <p className="text-sm text-amber-600">请先扫描文件</p>
      )}

      {organizeMutation.isPending && progress && (
        <div className="space-y-2">
          <div className="flex items-center justify-between text-sm">
            <div className="flex items-center gap-2 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              <span>
                整理中 {progress.processed} / {progress.total}
              </span>
            </div>
            <span className="font-medium">{percent}%</span>
          </div>
          <Progress value={percent} className="h-2" />
          <p className="text-xs text-muted-foreground truncate">
            {progress.current_file}
          </p>
        </div>
      )}

      {result && !organizeMutation.isPending && (
        <div className="flex items-center gap-2 flex-wrap">
          <Badge variant="secondary">成功 {result.success}</Badge>
          {result.skipped > 0 && (
            <Badge variant="outline">跳过 {result.skipped}</Badge>
          )}
        </div>
      )}

      {organizeMutation.isError && (
        <p className="text-sm text-destructive">
          整理失败: {organizeMutation.error?.message || "未知错误"}
        </p>
      )}

      <Button
        onClick={() => {
          setResult(null);
          organizeMutation.mutate();
        }}
        disabled={disabled}
        className="gap-2"
      >
        {organizeMutation.isPending ? (
          <>
            <Loader2 className="h-4 w-4 animate-spin" />
            整理中...
          </>
        ) : (
          <>
            <CalendarDays className="h-4 w-4" />
            开始整理
          </>
        )}
      </Button>
    </div>
  );
}
