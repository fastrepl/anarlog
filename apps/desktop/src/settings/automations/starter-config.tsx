import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { open as selectFolder } from "@tauri-apps/plugin-dialog";
import { type ReactNode } from "react";

import { FolderOpen } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn, formatDistanceToNow } from "@anlg/utils";

import {
  type AutomationRunRecord,
  parseAutomationRunRecord,
} from "~/automations/engine";
import { setSettingValue, useStoredSettingValue } from "~/settings/queries";
import { type SettingKey } from "~/settings/schema";

export function AutomationLastRunLine({
  settingKey,
  lastRun: lastRunOverride,
}: {
  settingKey?: SettingKey;
  lastRun?: AutomationRunRecord | null;
}) {
  const storedLastRun = parseAutomationRunRecord(
    useStoredSettingValue(settingKey ?? "automation_draft_template").value as
      | string
      | undefined,
  );
  const lastRun =
    lastRunOverride !== undefined ? lastRunOverride : storedLastRun;
  if (!lastRun) {
    return null;
  }
  const relative = formatDistanceToNow(new Date(lastRun.at), {
    addSuffix: true,
  });
  return (
    <p
      className={cn([
        "mt-3 truncate text-xs",
        lastRun.status === "error"
          ? "text-destructive"
          : "text-muted-foreground",
      ])}
      title={lastRun.detail}
    >
      {lastRun.status === "success" ? (
        <Trans>
          Last run {relative}: {lastRun.detail}
        </Trans>
      ) : (
        <Trans>
          Last run failed {relative}: {lastRun.detail}
        </Trans>
      )}
    </p>
  );
}

function ConfigRow({
  title,
  value,
  children,
}: {
  title: ReactNode;
  value: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div className="min-w-0">
        <h4 className="text-xs font-semibold">{title}</h4>
        <p className="text-muted-foreground mt-1 truncate text-xs">{value}</p>
      </div>
      {children}
    </div>
  );
}

export function MarkdownExportConfig({
  value,
  onChange,
}: {
  value?: string;
  onChange?: (directory: string) => void;
} = {}) {
  const { t } = useLingui();
  const storedDirectory = (
    useStoredSettingValue("automation_markdown_export_directory").value ?? ""
  ).trim();
  const directory = (value ?? storedDirectory).trim();
  const chooseFolderMutation = useMutation({
    mutationKey: ["automation-markdown-export-folder"],
    mutationFn: async () => {
      const selected = await selectFolder({
        title: t`Choose export folder`,
        directory: true,
        multiple: false,
        defaultPath: directory || undefined,
      });
      if (typeof selected === "string" && selected) {
        if (onChange) {
          onChange(selected);
          return;
        }
        await setSettingValue("automation_markdown_export_directory", selected);
      }
    },
    onError: () => sonnerToast.error(t`Could not update the export folder`),
  });

  return (
    <ConfigRow
      title={<Trans>Export folder</Trans>}
      value={directory || <Trans>No folder selected yet.</Trans>}
    >
      <Button
        type="button"
        size="sm"
        variant="outline"
        onClick={() => chooseFolderMutation.mutate()}
        disabled={chooseFolderMutation.isPending}
      >
        <FolderOpen size={14} />
        <Trans>Choose folder</Trans>
      </Button>
    </ConfigRow>
  );
}
