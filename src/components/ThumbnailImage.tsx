import { useQuery } from "@tanstack/react-query";
import { getThumbnail } from "@/lib/commands";
import { ImageOff, Loader2 } from "lucide-react";

interface ThumbnailImageProps {
  path: string;
  alt?: string;
  className?: string;
  onClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}

export default function ThumbnailImage({
  path,
  alt,
  className = "",
  onClick,
  onContextMenu,
}: ThumbnailImageProps) {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["thumbnail", path],
    queryFn: () => getThumbnail(path, 300),
    staleTime: Infinity,
    gcTime: 10 * 60 * 1000,
    retry: 2,
    retryDelay: 1000,
  });

  if (isLoading) {
    return (
      <div
        className={`flex items-center justify-center bg-muted/50 ${className}`}
      >
        <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (isError || !data) {
    return (
      <div
        className={`flex items-center justify-center bg-muted/50 ${className}`}
      >
        <ImageOff className="h-5 w-5 text-muted-foreground" />
      </div>
    );
  }

  return (
    <img
      src={data}
      alt={alt || path.split(/[/\\]/).pop() || "thumbnail"}
      className={`object-cover ${className}`}
      onClick={onClick}
      onContextMenu={onContextMenu}
      draggable={false}
    />
  );
}
