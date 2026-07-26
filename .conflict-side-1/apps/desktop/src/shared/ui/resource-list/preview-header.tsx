import { useLingui } from "@lingui/react/macro";
import { Copy } from "lucide-react";
import type { ReactNode } from "react";

import { Button, type ButtonProps } from "@hypr/ui/components/ui/button";

import { TemplateCategoryLabel } from "../template-category-label";

import { getTemplateCreatorLabel } from "~/templates/utils";

export function ResourcePreviewHeader({
  title,
  description,
  category,
  targets,
  onClone,
  actionLabel,
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
  const { t } = useLingui();
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
      {actionLabel ?? t`Clone`}
    </Button>
  ) : null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex h-12 items-center justify-between gap-3 pr-1 pl-3">
        <div className="min-w-0">
          <TemplateCategoryLabel category={category} />
        </div>
        <div className="flex items-center gap-0">
          {actions}
          {actionButton}
        </div>
      </div>

      <div className="scroll-fade-y min-h-0 flex-1 overflow-y-auto px-6 pt-3 pb-6">
        <div className="min-w-0">
          <div className="flex min-w-0 items-baseline gap-2">
            <h2 className="min-w-0 truncate text-lg font-semibold">
              {title || t`Untitled`}
            </h2>
            {titleMeta}
          </div>
          {description && (
            <p className="text-muted-foreground mt-1 min-h-[24px] text-sm">
              {description}
            </p>
          )}
          {targets && targets.length > 0 && (
            <div className="mt-2 flex min-h-6 flex-wrap items-center gap-1.5">
              {targets.map((target, index) => (
                <span
                  key={index}
                  className="bg-muted text-muted-foreground inline-flex h-6 items-center rounded-md px-2 py-0.5 text-xs"
                >
                  {target}
                </span>
              ))}
            </div>
          )}
          {footer === undefined ? (
            <p className="text-muted-foreground mt-2 text-xs">
              {getTemplateCreatorLabel({ isUserTemplate: false })}
            </p>
          ) : (
            footer
          )}
        </div>
        {children}
      </div>
    </div>
  );
}
