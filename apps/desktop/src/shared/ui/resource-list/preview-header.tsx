import { Copy } from "lucide-react";
import type { ReactNode } from "react";

import { Button, type ButtonProps } from "@hypr/ui/components/ui/button";

import { TemplateCategoryLabel } from "../template-category-label";

import { getTemplateCreatorLabel } from "~/templates/shared";

export function ResourcePreviewHeader({
  title,
  description,
  category,
  targets,
  onClone,
  actionLabel = "Clone",
  actionIcon,
  actionVariant,
  actionClassName,
  actions,
  titleMeta,
  footer,
  children,
}: {
  title: string;
  description?: string | null;
  category?: string | null;
  targets?: string[] | null;
  onClone?: () => void;
  actionLabel?: string;
  actionIcon?: ReactNode;
  actionVariant?: ButtonProps["variant"];
  actionClassName?: string;
  actions?: ReactNode;
  titleMeta?: ReactNode;
  footer?: ReactNode;
  children?: ReactNode;
}) {
  const actionButton = onClone ? (
    <Button
      onClick={onClone}
      size="sm"
      variant={actionVariant}
      className={actionClassName}
    >
      {actionIcon === undefined ? (
        <Copy className="mr-2 h-4 w-4" />
      ) : (
        actionIcon
      )}
      {actionLabel}
    </Button>
  ) : null;

  return (
    <div className="pt-1 pr-1 pb-4 pl-3">
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <TemplateCategoryLabel category={category} />
        </div>
        <div className="flex items-center gap-0">
          {actions}
          {actionButton}
        </div>
      </div>
      <div className="mt-3 min-w-0 pr-5 pl-3">
        <div className="flex min-w-0 items-baseline gap-2">
          <h2 className="min-w-0 truncate text-lg font-semibold">
            {title || "Untitled"}
          </h2>
          {titleMeta}
        </div>
        {description && (
          <p className="mt-1 min-h-[24px] text-sm text-neutral-500">
            {description}
          </p>
        )}
        {targets && targets.length > 0 && (
          <div className="mt-2 flex min-h-6 flex-wrap items-center gap-1.5">
            {targets.map((target, index) => (
              <span
                key={index}
                className="inline-flex h-6 items-center rounded-md bg-neutral-100 px-2 py-0.5 text-xs text-neutral-600"
              >
                {target}
              </span>
            ))}
          </div>
        )}
        {footer === undefined ? (
          <p className="mt-2 text-xs text-neutral-400">
            {getTemplateCreatorLabel({ isUserTemplate: false })}
          </p>
        ) : (
          footer
        )}
      </div>
      {children}
    </div>
  );
}
