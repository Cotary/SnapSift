import { useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  getProjectDetail,
  addSourceFolders,
  removeSourceFolder,
  setTargetDir,
  startScan,
  getProjectFiles,
} from "@/lib/commands";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import FolderList from "@/components/FolderList";
import ScanProgress from "@/components/ScanProgress";
import OrganizePanel from "@/components/OrganizePanel";
import type { ScanResult } from "@/lib/types";
import {
  ArrowLeft,
  FolderPlus,
  FolderOutput,
  FolderOpen,
  ScanSearch,
  Images,
  ImageIcon,
  Film,
  Music,
  Loader2,
} from "lucide-react";

export default function ProjectDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);

  const { data, isLoading, error } = useQuery({
    queryKey: ["project", id],
    queryFn: () => getProjectDetail(id!),
    enabled: !!id,
  });

  const { data: files = [] } = useQuery({
    queryKey: ["project-files", id],
    queryFn: () => getProjectFiles(id!),
    enabled: !!id,
  });

  const addFoldersMutation = useMutation({
    mutationFn: (paths: string[]) => addSourceFolders(id!, paths),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["project", id] });
    },
  });

  const removeFolderMutation = useMutation({
    mutationFn: removeSourceFolder,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["project", id] });
    },
  });

  const setTargetMutation = useMutation({
    mutationFn: (path: string) => setTargetDir(id!, path),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["project", id] });
    },
  });

  const scanMutation = useMutation({
    mutationFn: () => startScan(id!),
    onSuccess: (result) => {
      setScanResult(result);
      queryClient.invalidateQueries({ queryKey: ["project-files", id] });
    },
  });

  const handleAddSourceFolders = async () => {
    const selected = await open({
      directory: true,
      multiple: true,
      title: "选择源文件夹",
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length > 0) {
        addFoldersMutation.mutate(paths);
      }
    }
  };

  const handleSetTargetDir = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择目标文件夹",
    });
    if (selected && typeof selected === "string") {
      setTargetMutation.mutate(selected);
    }
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <p className="text-muted-foreground">加载中...</p>
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="flex flex-col items-center justify-center min-h-screen gap-4">
        <p className="text-destructive">项目未找到</p>
        <Button variant="outline" onClick={() => navigate("/")}>
          返回列表
        </Button>
      </div>
    );
  }

  const { project, source_folders } = data;

  // Split files into source files and target files
  const sourceFiles = project.target_dir
    ? files.filter((f) => {
        const normalizedPath = f.path.replace(/\\/g, "/");
        const normalizedTarget = project.target_dir!.replace(/\\/g, "/");
        return !normalizedPath.startsWith(normalizedTarget);
      })
    : files;

  const imageCount = sourceFiles.filter((f) => f.file_type === "image").length;
  const videoCount = sourceFiles.filter((f) => f.file_type === "video").length;
  const audioCount = sourceFiles.filter((f) => f.file_type === "audio").length;

  return (
    <div className="container mx-auto max-w-4xl px-6 py-10">
      <header className="flex items-center gap-4 mb-8">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate("/")}
          className="shrink-0"
        >
          <ArrowLeft className="h-5 w-5" />
        </Button>
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{project.name}</h1>
          <p className="text-sm text-muted-foreground">
            创建于 {new Date(project.created_at).toLocaleDateString("zh-CN")}
          </p>
        </div>
      </header>

      <div className="grid gap-6">
        {/* Source Folders */}
        <Card>
          <CardHeader className="flex-row items-center justify-between space-y-0">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-base">
                <FolderPlus className="h-5 w-5" />
                源文件夹
              </CardTitle>
              <CardDescription>
                添加需要扫描的图片/视频所在文件夹
              </CardDescription>
            </div>
            <Button
              onClick={handleAddSourceFolders}
              disabled={addFoldersMutation.isPending}
              className="gap-2"
            >
              <FolderPlus className="h-4 w-4" />
              {addFoldersMutation.isPending ? "添加中..." : "添加文件夹"}
            </Button>
          </CardHeader>
          <CardContent>
            <FolderList
              folders={source_folders}
              onRemove={(folderId) => removeFolderMutation.mutate(folderId)}
              isRemoving={removeFolderMutation.isPending}
            />
          </CardContent>
        </Card>

        {/* Target Folder */}
        <Card>
          <CardHeader className="flex-row items-center justify-between space-y-0">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-base">
                <FolderOutput className="h-5 w-5" />
                目标文件夹
              </CardTitle>
              <CardDescription>整理后的文件将归档到此文件夹</CardDescription>
            </div>
            <Button
              variant="outline"
              onClick={handleSetTargetDir}
              disabled={setTargetMutation.isPending}
              className="gap-2"
            >
              <FolderOpen className="h-4 w-4" />
              {setTargetMutation.isPending ? "设置中..." : "选择文件夹"}
            </Button>
          </CardHeader>
          <CardContent>
            {project.target_dir ? (
              <div className="flex items-center gap-3 rounded-lg border px-4 py-3 bg-muted/30">
                <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
                <span className="text-sm truncate" title={project.target_dir}>
                  {project.target_dir}
                </span>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
                <FolderOutput className="h-10 w-10 mb-2 opacity-40" />
                <p className="text-sm">尚未设置目标文件夹</p>
              </div>
            )}
          </CardContent>
        </Card>

        {/* Scan */}
        <Card>
          <CardHeader className="flex-row items-center justify-between space-y-0">
            <div className="space-y-1">
              <CardTitle className="flex items-center gap-2 text-base">
                <ScanSearch className="h-5 w-5" />
                文件扫描
              </CardTitle>
              <CardDescription>
                扫描源文件夹中的图片和视频，提取元数据与指纹
              </CardDescription>
            </div>
            <Button
              onClick={() => scanMutation.mutate()}
              disabled={scanMutation.isPending || source_folders.length === 0}
              className="gap-2"
            >
              {scanMutation.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <ScanSearch className="h-4 w-4" />
              )}
              {scanMutation.isPending ? "扫描中..." : "开始扫描"}
            </Button>
          </CardHeader>
          <CardContent>
            <ScanProgress isScanning={scanMutation.isPending} />

            {(scanResult || sourceFiles.length > 0) && !scanMutation.isPending && (
              <div className="flex items-center gap-3 flex-wrap pt-2">
                <Badge variant="secondary" className="gap-1">
                  <ImageIcon className="h-3 w-3" />
                  图片 {scanResult?.images ?? imageCount}
                </Badge>
                <Badge variant="secondary" className="gap-1">
                  <Film className="h-3 w-3" />
                  视频 {scanResult?.videos ?? videoCount}
                </Badge>
                <Badge variant="secondary" className="gap-1">
                  <Music className="h-3 w-3" />
                  音频 {scanResult?.audios ?? audioCount}
                </Badge>
                <Badge variant="outline" className="gap-1">
                  总计 {scanResult?.total_files ?? sourceFiles.length} 个文件
                </Badge>
              </div>
            )}

            {source_folders.length === 0 && (
              <p className="text-sm text-amber-600 py-2">
                请先添加源文件夹
              </p>
            )}
          </CardContent>
        </Card>

        {/* Organize */}
        {sourceFiles.length > 0 && (
          <Card>
            <CardContent className="pt-6">
              <OrganizePanel
                projectId={id!}
                hasTargetDir={!!project.target_dir}
                hasFiles={sourceFiles.length > 0}
              />
            </CardContent>
          </Card>
        )}

        {/* Dedup Entry */}
        <Card
          className="cursor-pointer transition-colors hover:bg-muted/50"
          onClick={() => navigate(`/project/${id}/dedup`)}
        >
          <CardContent className="flex items-center gap-4 py-6">
            <Images className="h-8 w-8 text-primary opacity-80 shrink-0" />
            <div>
              <p className="font-medium">相似图片筛选</p>
              <p className="text-sm text-muted-foreground">
                整理文件后，查找相似图片并批量去重
              </p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
