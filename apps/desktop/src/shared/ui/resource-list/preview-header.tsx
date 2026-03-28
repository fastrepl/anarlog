import { Copy } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@hypr/ui/components/ui/button";

export function ResourcePreviewHeader({
  title,
  description,
  category,
  targets,
  onClone,
  actionLabel = "Clone",
  actionIcon,
  children,
}: {
  title: string;
  description?: string | null;
  category?: string | null;
  targets?: string[] | null;
  onClone: () => void;
  actionLabel?: string;
  actionIcon?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="px-6 pt-1 pb-4">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          {category ? (
            <span className="font-mono text-xs text-stone-400">{category}</span>
          ) : null}
        </div>
        <Button onClick={onClone} size="sm" className="shrink-0">
          {actionIcon ?? <Copy className="mr-2 h-4 w-4" />}
          {actionLabel}
        </Button>
      </div>
      <div className="mt-3 min-w-0">
        <h2 className="truncate text-lg font-semibold">
          {title || "Untitled"}
        </h2>
        {description && (
          <p className="mt-1 text-sm text-neutral-500">{description}</p>
        )}
      </div>
      {targets && targets.length > 0 && (
        <div className="mt-2 flex flex-wrap items-center gap-2">
          {targets.map((target, index) => (
            <span
              key={index}
              className="rounded-xs bg-neutral-100 px-2 py-0.5 text-xs text-neutral-600"
            >
              {target}
            </span>
          ))}
        </div>
      )}
      {children}
    </div>
  );
}
