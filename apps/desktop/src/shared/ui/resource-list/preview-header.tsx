import { useLingui } from "@lingui/react/macro";
import { Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors, radii, spacing } from "@anlg/design-system/tokens.stylex";
import { Button, type ButtonProps } from "@anlg/ui/components/ui/button";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

import { getTemplateCreatorLabel } from "~/templates/utils";

export function ResourcePreviewHeader({
  icon,
  title,
  description,
  targets,
  onClone,
  actionLabel,
  actionIcon,
  actionVariant,
  actionSx,
  actions,
  titleMeta,
  footer,
  children,
}: {
  icon?: ReactNode;
  title: string;
  description?: string | null;
  targets?: string[] | null;
  onClone?: () => void;
  actionLabel?: string;
  actionIcon?: ReactNode;
  actionVariant?: ButtonProps["variant"];
  actionSx?: StyleXProps["sx"];
  actions?: ReactNode;
  titleMeta?: ReactNode;
  footer?: ReactNode;
  children?: ReactNode;
}) {
  const { t } = useLingui();
  const actionButton = onClone ? (
    <Button onClick={onClone} size="sm" variant={actionVariant} sx={actionSx}>
      {actionIcon === undefined ? (
        <Copy {...stylex.props(styles.copyIcon)} />
      ) : (
        actionIcon
      )}
      {actionLabel ?? t`Clone`}
    </Button>
  ) : null;

  return (
    <div {...stylex.props(styles.root)}>
      <div {...stylex.props(styles.header)}>
        <div {...stylex.props(styles.titleRow)}>
          {icon}
          <h2 {...stylex.props(styles.title)}>{title || t`Untitled`}</h2>
          {titleMeta}
        </div>
        <div {...stylex.props(styles.actions)}>
          {actions}
          {actionButton}
        </div>
      </div>

      <div {...mergeStyleXProps(styles.scrollContent, "scroll-fade-y")}>
        <div {...stylex.props(styles.content)}>
          {description && (
            <p {...stylex.props(styles.description)}>{description}</p>
          )}
          {targets && targets.length > 0 && (
            <div {...stylex.props(styles.targets)}>
              {targets.map((target, index) => (
                <span key={index} {...stylex.props(styles.target)}>
                  {target}
                </span>
              ))}
            </div>
          )}
          {footer === undefined ? (
            <p {...stylex.props(styles.footer)}>
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

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    gap: 0,
  },
  content: {
    minWidth: 0,
  },
  copyIcon: {
    height: "1rem",
    marginRight: spacing.sm,
    width: "1rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    minHeight: "24px",
  },
  footer: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: spacing.sm,
  },
  header: {
    alignItems: "center",
    display: "flex",
    gap: spacing.md,
    height: "3rem",
    justifyContent: "space-between",
    paddingLeft: spacing.md,
    paddingRight: spacing.xs,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  scrollContent: {
    flex: "1",
    minHeight: 0,
    overflowY: "auto",
    paddingBottom: spacing.xl,
    paddingInline: spacing.xl,
    paddingTop: spacing.md,
  },
  target: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    color: colors.mutedForeground,
    display: "inline-flex",
    fontSize: "0.75rem",
    height: "1.5rem",
    paddingBlock: "0.125rem",
    paddingInline: spacing.sm,
  },
  targets: {
    alignItems: "center",
    display: "flex",
    flexWrap: "wrap",
    gap: "0.375rem",
    marginTop: spacing.sm,
    minHeight: "1.5rem",
  },
  title: {
    fontSize: "0.875rem",
    fontWeight: 600,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  titleRow: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: spacing.sm,
    minWidth: 0,
  },
});
