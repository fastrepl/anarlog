import { Icon } from "@iconify-icon/react";
import MuxPlayer from "@mux/mux-player-react";

import { cn } from "@hypr/utils";

interface VideoThumbnailProps {
  playbackId: string;
  className?: string;
  onPlay?: () => void;
}

export function VideoThumbnail({
  playbackId,
  className,
  onPlay,
}: VideoThumbnailProps) {
  return (
    <div
      className={cn([
        "group relative h-full w-full cursor-pointer overflow-hidden",
        className,
      ])}
      onClick={onPlay}
    >
      <MuxPlayer
        playbackId={playbackId}
        poster="https://image.mux.com/bpcBHf4Qv5FbhwWD02zyFDb24EBuEuTPHKFUrZEktULQ/thumbnail.png?width=1152&height=648&time=58.6"
        muted
        playsInline
        className="pointer-events-none aspect-video h-full w-full object-cover"
        style={
          {
            "--controls": "none",
          } as React.CSSProperties & { [key: `--${string}`]: string }
        }
      />

      <div className="absolute inset-0 flex items-center justify-center bg-black/20 transition-all duration-200 group-hover:bg-black/30">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onPlay?.();
          }}
          className={cn([
            "size-16 rounded-full bg-white/90 backdrop-blur-xs",
            "flex items-center justify-center",
            "transition-all duration-200 hover:scale-110 hover:bg-white",
            "cursor-pointer shadow-xl",
          ])}
          aria-label="Play video"
        >
          <Icon icon="mdi:play" className="text-4xl text-stone-700" />
        </button>
      </div>
    </div>
  );
}
