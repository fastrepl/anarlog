import { Copy } from "lucide-react";
import type { ReactNode } from "react";

import { Button } from "@hypr/ui/components/ui/button";

export function ResourcePreviewHeader({
  title,
  description,
  category,
  targets,
  onClone,
  children,
}: {
  title: string;
  description?: string | null;
  category?: string | null;
  targets?: string[] | null;
  onClone: () => void;
  children?: ReactNode;
}) {
  return (
    <div className="border-b border-neutral-200 px-6 py-4">
      <div className="flex items-center justify-between">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-lg font-semibold">
            {title || "Untitled"}
          </h2>
          {description && (
            <p className="mt-1 text-sm text-neutral-500">{description}</p>
          )}
        </div>
        <Button onClick={onClone} size="sm" className="ml-4 shrink-0">
          <Copy className="mr-2 h-4 w-4" />
          Clone
        </Button>
      </div>
      {category && (
        <div className="mt-2">
          <span className="font-mono text-xs text-stone-400">({category})</span>
        </div>
      )}
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
