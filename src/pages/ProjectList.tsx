import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { listProjects, createProject, deleteProject } from "@/lib/commands";
import { Card, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import CreateProjectDialog from "@/components/CreateProjectDialog";
import { Camera, FolderOpen, Trash2, ChevronRight } from "lucide-react";
import type { Project } from "@/lib/types";

export default function ProjectList() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: projects = [], isLoading } = useQuery({
    queryKey: ["projects"],
    queryFn: listProjects,
  });

  const createMutation = useMutation({
    mutationFn: createProject,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteProject,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
  });

  const handleDelete = (e: React.MouseEvent, projectId: string) => {
    e.stopPropagation();
    deleteMutation.mutate(projectId);
  };

  return (
    <div className="container mx-auto max-w-4xl px-6 py-10">
      <header className="flex items-center justify-between mb-10">
        <div className="flex items-center gap-3">
          <Camera className="h-8 w-8 text-primary" />
          <div>
            <h1 className="text-2xl font-bold tracking-tight">Realphoto</h1>
            <p className="text-sm text-muted-foreground">
              多媒体整理与去重工具
            </p>
          </div>
        </div>
        <CreateProjectDialog
          onSubmit={(name) => createMutation.mutate(name)}
          isPending={createMutation.isPending}
        />
      </header>

      {isLoading ? (
        <div className="flex items-center justify-center py-20">
          <p className="text-muted-foreground">加载中...</p>
        </div>
      ) : projects.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-muted-foreground">
          <FolderOpen className="h-16 w-16 mb-4 opacity-30" />
          <p className="text-lg font-medium">还没有项目</p>
          <p className="text-sm mt-1">点击右上角按钮创建你的第一个项目</p>
        </div>
      ) : (
        <div className="grid gap-4">
          {projects.map((project: Project) => (
            <Card
              key={project.id}
              className="cursor-pointer transition-colors hover:bg-muted/50 group"
              onClick={() => navigate(`/project/${project.id}`)}
            >
              <CardHeader className="flex-row items-center justify-between space-y-0">
                <div className="space-y-1">
                  <CardTitle className="text-lg">{project.name}</CardTitle>
                  <CardDescription>
                    创建于{" "}
                    {new Date(project.created_at).toLocaleDateString("zh-CN")}
                    {project.target_dir && (
                      <span className="ml-3">
                        目标: {project.target_dir}
                      </span>
                    )}
                  </CardDescription>
                </div>
                <div className="flex items-center gap-2">
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-8 w-8 text-muted-foreground hover:text-destructive opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={(e) => handleDelete(e, project.id)}
                    disabled={deleteMutation.isPending}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                  <ChevronRight className="h-5 w-5 text-muted-foreground" />
                </div>
              </CardHeader>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
