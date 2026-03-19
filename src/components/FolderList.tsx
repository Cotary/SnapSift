import type { SourceFolder } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { FolderOpen, Trash2 } from "lucide-react";

interface FolderListProps {
  folders: SourceFolder[];
  onRemove: (folderId: string) => void;
  isRemoving: boolean;
}

export default function FolderList({
  folders,
  onRemove,
  isRemoving,
}: FolderListProps) {
  if (folders.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-muted-foreground">
        <FolderOpen className="h-12 w-12 mb-3 opacity-40" />
        <p className="text-sm">尚未添加源文件夹</p>
        <p className="text-xs mt-1">点击上方按钮添加要扫描的文件夹</p>
      </div>
    );
  }

  return (
    <ScrollArea className="max-h-[320px]">
      <div className="space-y-2">
        {folders.map((folder) => (
          <div
            key={folder.id}
            className="flex items-center justify-between gap-3 rounded-lg border px-4 py-3 bg-muted/30"
          >
            <div className="flex items-center gap-3 min-w-0">
              <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
              <span className="text-sm truncate" title={folder.path}>
                {folder.path}
              </span>
            </div>
            <Button
              variant="ghost"
              size="icon"
              className="shrink-0 h-8 w-8 text-muted-foreground hover:text-destructive"
              onClick={() => onRemove(folder.id)}
              disabled={isRemoving}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        ))}
      </div>
    </ScrollArea>
  );
}
