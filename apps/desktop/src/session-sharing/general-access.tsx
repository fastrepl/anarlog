import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { Buildings, CircleNotch, Globe, LockKey } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@anlg/ui/components/ui/select";

import type { AvailableShareWorkspace } from "./source";

export type GeneralAccessTarget = "restricted" | "link" | `workspace:${string}`;
export type GeneralAccessValue = GeneralAccessTarget | "public";

export function GeneralAccessSelector({
  value,
  workspaces,
  disabled,
  canExpand,
  pending,
  allowedScopes = ["restricted", "workspace", "link", "public"],
  onValueChange,
}: {
  value: GeneralAccessValue;
  workspaces: AvailableShareWorkspace[];
  disabled: boolean;
  canExpand: boolean;
  pending: boolean;
  allowedScopes?: Array<"restricted" | "workspace" | "link" | "public">;
  onValueChange: (value: GeneralAccessTarget) => void;
}) {
  const AccessIcon =
    value === "restricted"
      ? LockKey
      : value.startsWith("workspace:")
        ? Buildings
        : Globe;

  return (
    <div {...stylex.props(styles.root)}>
      <span {...stylex.props(styles.iconContainer)}>
        {pending ? (
          <CircleNotch
            {...stylex.props(styles.icon, styles.spinner)}
            aria-hidden="true"
          />
        ) : (
          <AccessIcon {...stylex.props(styles.icon)} aria-hidden="true" />
        )}
      </span>
      <Select
        value={value}
        disabled={disabled || pending}
        onValueChange={(nextValue) => {
          const target = resolveGeneralAccessTarget(nextValue, workspaces);
          if (target) onValueChange(target);
        }}
      >
        <SelectTrigger aria-label={t`General access`} sx={styles.trigger}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectItem value="restricted">
            <Trans>Only people invited</Trans>
          </SelectItem>
          {workspaces.map((workspace) => (
            <SelectItem
              key={workspace.id}
              value={`workspace:${workspace.id}`}
              disabled={!canExpand || !allowedScopes.includes("workspace")}
            >
              <Trans>Everyone in {workspace.name}</Trans>
            </SelectItem>
          ))}
          <SelectItem
            value="link"
            disabled={!canExpand || !allowedScopes.includes("link")}
          >
            <Trans>Anyone with the link</Trans>
          </SelectItem>
          {value === "public" ? (
            <>
              <SelectSeparator />
              <SelectItem
                value="public"
                disabled={!allowedScopes.includes("public")}
              >
                <Trans>Public on the web</Trans>
              </SelectItem>
            </>
          ) : null}
        </SelectContent>
      </Select>
    </div>
  );
}

export function resolveGeneralAccessTarget(
  value: string,
  workspaces: AvailableShareWorkspace[],
): GeneralAccessTarget | null {
  if (value === "restricted" || value === "link") return value;
  if (!value.startsWith("workspace:")) return null;
  const workspaceId = value.slice("workspace:".length);
  return workspaces.some((workspace) => workspace.id === workspaceId)
    ? `workspace:${workspaceId}`
    : null;
}

export function generalAccessWorkspaceId(
  target: GeneralAccessTarget,
  workspaces: AvailableShareWorkspace[],
) {
  if (!target.startsWith("workspace:")) return null;
  const workspaceId = target.slice("workspace:".length);
  return workspaces.some((workspace) => workspace.id === workspaceId)
    ? workspaceId
    : null;
}

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  icon: {
    height: "1rem",
    width: "1rem",
  },
  iconContainer: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    display: "flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    width: "1.75rem",
  },
  root: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.375rem",
    minWidth: 0,
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  trigger: {
    backgroundColor: "transparent",
    borderRadius: radii.md,
    borderWidth: 0,
    boxShadow: "none",
    flexGrow: "0",
    flexShrink: "1",
    flexBasis: "auto",
    fontSize: "0.75rem",
    gap: "0.25rem",
    height: "1.75rem",
    justifyContent: "flex-start",
    maxWidth: "100%",
    minWidth: 0,
    paddingInline: "0.375rem",
    width: "auto",
  },
});
