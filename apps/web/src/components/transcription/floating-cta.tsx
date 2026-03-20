import { useRef } from "react";

import { cn } from "@hypr/utils";

const pillClasses = cn([
  "flex items-center gap-2 px-5 py-2.5",
  "rounded-full border-2 border-neutral-200 bg-white",
  "shadow-lg",
]);

export function FloatingCTA({
  status,
  progress,
  onFileSelect,
}: {
  status: string;
  progress: number;
  onFileSelect: (e: React.ChangeEvent<HTMLInputElement>) => void;
}) {
  const fileInputRef = useRef<HTMLInputElement>(null);

  return (
    <div className="absolute bottom-6 left-1/2 z-40 -translate-x-1/2">
      <input
        ref={fileInputRef}
        type="file"
        accept="audio/*"
        onChange={onFileSelect}
        className="hidden"
      />

      {status === "uploading" ? (
        <div className={pillClasses}>
          <div className="h-4 w-4 animate-spin rounded-full border-2 border-stone-300 border-t-stone-600" />
          <span className="text-sm font-medium text-neutral-700">
            Uploading... {progress}%
          </span>
        </div>
      ) : status === "transcribing" ? (
        <div className={pillClasses}>
          <div className="h-4 w-4 animate-spin rounded-full border-2 border-stone-300 border-t-stone-600" />
          <span className="text-sm font-medium text-neutral-700">
            Transcribing...
          </span>
        </div>
      ) : (
        <button
          onClick={() => fileInputRef.current?.click()}
          className={cn([
            "flex items-center gap-2 px-5 py-2.5",
            "rounded-full bg-neutral-900",
            "shadow-lg",
            "hover:bg-neutral-800",
            "active:scale-[98%]",
            "transition-all",
          ])}
        >
          <span className="flex h-2.5 w-2.5 rounded-full bg-red-400" />
          <span className="text-sm font-medium text-white">Upload file</span>
        </button>
      )}
    </div>
  );
}
