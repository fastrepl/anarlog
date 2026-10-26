import type { HTMLAttributes, ReactNode } from "react";

import { FileText, Waveform } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { AUDIO_EXTENSIONS } from "~/stt/useUploadFile";

const supportedAudioFormats = formatAudioExtensionList(AUDIO_EXTENSIONS);

export function AudioDropTarget({
  children,
  className,
  isActive,
  targetProps,
}: {
  children: ReactNode;
  className?: string;
  isActive: boolean;
  targetProps: HTMLAttributes<HTMLDivElement>;
}) {
  return (
    <FileDropTarget
      targetProps={targetProps}
      className={className}
      isActive={isActive}
      icon={<Waveform className="text-muted-foreground size-5 shrink-0" />}
      title="Drop to upload and transcribe audio"
      subtitle={`${supportedAudioFormats} audio`}
    >
      {children}
    </FileDropTarget>
  );
}

export function MarkdownDropTarget({
  children,
  className,
  isActive,
  title,
  subtitle,
  targetProps,
}: {
  children: ReactNode;
  className?: string;
  isActive: boolean;
  title: string;
  subtitle: string;
  targetProps: HTMLAttributes<HTMLDivElement>;
}) {
  return (
    <FileDropTarget
      targetProps={targetProps}
      className={className}
      isActive={isActive}
      icon={<FileText className="text-muted-foreground size-5 shrink-0" />}
      title={title}
      subtitle={subtitle}
    >
      {children}
    </FileDropTarget>
  );
}

function FileDropTarget({
  children,
  className,
  isActive,
  icon,
  title,
  subtitle,
  targetProps,
}: {
  children: ReactNode;
  className?: string;
  isActive: boolean;
  icon: ReactNode;
  title: string;
  subtitle: string;
  targetProps: HTMLAttributes<HTMLDivElement>;
}) {
  return (
    <div {...targetProps} className={cn(["relative min-h-full", className])}>
      {isActive && (
        <div
          role="status"
          className={cn([
            "pointer-events-none absolute inset-0 z-30 flex items-center justify-center rounded-lg border border-dashed",
            "border-border/70 bg-background text-muted-foreground shadow-inner",
            "[background-image:radial-gradient(circle_at_center,_rgba(113,113,122,0.34)_1px,_transparent_1px)]",
            "[background-size:18px_18px]",
          ])}
        >
          <div className="border-border/70 bg-card/95 text-foreground flex items-center gap-3 rounded-md border px-4 py-3 shadow-sm">
            {icon}
            <div className="flex min-w-0 flex-col gap-0.5">
              <p className="text-sm font-medium">{title}</p>
              <p className="text-muted-foreground text-xs">{subtitle}</p>
            </div>
          </div>
        </div>
      )}
      {children}
    </div>
  );
}

function formatAudioExtensionList(extensions: string[]) {
  const labels = extensions.map((extension) => extension.toUpperCase());
  if (labels.length <= 1) {
    return labels.join("");
  }

  return `${labels.slice(0, -1).join(", ")}, or ${labels[labels.length - 1]}`;
}
