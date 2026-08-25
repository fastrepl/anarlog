import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { Buildings, CircleNotch, Globe, LockKey } from "@phosphor-icons/react";

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
    <div className="flex items-center gap-2 rounded-lg px-1.5 py-1">
      <span className="bg-muted flex size-7 shrink-0 items-center justify-center rounded-md">
        {pending ? (
          <CircleNotch className="size-4 animate-spin" aria-hidden="true" />
        ) : (
          <AccessIcon className="size-4" aria-hidden="true" />
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
        <SelectTrigger
          aria-label={t`General access`}
          className="h-8 min-w-0 flex-1 rounded-md border-0 bg-transparent px-2 text-xs shadow-none"
        >
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
