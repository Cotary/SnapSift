import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Progress } from "@/components/ui/progress";
import type { ScanProgress as ScanProgressData } from "@/lib/types";
import { Loader2 } from "lucide-react";

interface ScanProgressProps {
  isScanning: boolean;
}

export default function ScanProgress({ isScanning }: ScanProgressProps) {
  const [progress, setProgress] = useState<ScanProgressData | null>(null);

  useEffect(() => {
    if (!isScanning) {
      setProgress(null);
      return;
    }

    const unlisten = listen<ScanProgressData>("scan-progress", (event) => {
      setProgress(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [isScanning]);

  if (!isScanning || !progress) {
    if (isScanning) {
      return (
        <div className="flex items-center gap-2 text-sm text-muted-foreground py-3">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span>正在准备扫描...</span>
        </div>
      );
    }
    return null;
  }

  const percent =
    progress.total > 0 ? Math.round((progress.scanned / progress.total) * 100) : 0;

  const fileName = progress.current_file.split(/[/\\]/).pop() || progress.current_file;

  return (
    <div className="space-y-2 py-3">
      <div className="flex items-center justify-between text-sm">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span>
            扫描中 {progress.scanned} / {progress.total}
          </span>
        </div>
        <span className="font-medium">{percent}%</span>
      </div>
      <Progress value={percent} className="h-2" />
      <p className="text-xs text-muted-foreground truncate" title={progress.current_file}>
        {fileName}
      </p>
    </div>
  );
}
