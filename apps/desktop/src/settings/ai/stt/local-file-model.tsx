import { Trans, useLingui } from "@lingui/react/macro";
import { Check, CircleNotch, FolderOpen, X } from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open as selectFile } from "@tauri-apps/plugin-dialog";

import { commands as localSttCommands } from "@anlg/plugin-local-stt";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import type { HealthStatus } from "./health";

import { setSettingValue, setSettingValues } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";

export function LocalFileModel({
  healthStatus,
}: {
  healthStatus: HealthStatus["status"];
}) {
  const { t } = useLingui();
  const modelPath = useConfigValue("local_stt_model_path")?.trim() ?? "";
  const modelInfo = useQuery({
    queryKey: ["local-stt-model-file", modelPath],
    enabled: !!modelPath,
    queryFn: async () => {
      const result = await localSttCommands.inspectCustomModelPath(modelPath);
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    staleTime: Infinity,
  });

  const chooseModel = useMutation({
    mutationKey: ["choose-local-stt-model"],
    mutationFn: async () => {
      const selected = await selectFile({
        title: t`Choose a transcription model`,
        multiple: false,
        directory: false,
        defaultPath: modelPath || undefined,
        filters: [
          {
            name: t`Local transcription models`,
            extensions: ["bin", "gguf"],
          },
        ],
      });
      if (typeof selected !== "string" || !selected) {
        return;
      }

      const inspected = await localSttCommands.inspectCustomModelPath(selected);
      if (inspected.status === "error") {
        throw new Error(inspected.error);
      }

      const started = await localSttCommands.startServerForPath(
        inspected.data.path,
      );
      if (started.status === "error") {
        throw new Error(started.error);
      }

      await setSettingValues({
        current_stt_provider: "local_file",
        current_stt_model: "local-file",
        local_stt_model_path: inspected.data.path,
      });
    },
    onError: (error) => {
      sonnerToast.error(t`Could not use the selected model`, {
        description: error instanceof Error ? error.message : String(error),
      });
    },
  });

  const clearModel = useMutation({
    mutationKey: ["clear-local-stt-model"],
    mutationFn: () => setSettingValue("local_stt_model_path", ""),
    onError: () => sonnerToast.error(t`Could not clear the selected model`),
  });

  const pathParts = modelPath.split(/[/\\]/).filter(Boolean);
  const filename = modelInfo.data?.name || pathParts[pathParts.length - 1];
  const isPending = chooseModel.isPending || clearModel.isPending;

  return (
    <div className="border-input bg-card flex h-9 min-w-0 items-center gap-2 rounded-full border px-3 text-left shadow-none">
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
        title={modelPath || undefined}
        disabled={isPending}
        onClick={() => chooseModel.mutate()}
      >
        {chooseModel.isPending ? (
          <CircleNotch className="size-4 shrink-0 animate-spin" />
        ) : (
          <FolderOpen className="size-4 shrink-0" />
        )}
        <span className="min-w-0 flex-1 truncate text-sm">
          {filename || <Trans>Choose model file</Trans>}
        </span>
        {modelInfo.data ? (
          <span className="text-muted-foreground shrink-0 text-[11px]">
            {formatModelSize(modelInfo.data.sizeBytes)} · GGML ·{" "}
            <Trans>After recording</Trans>
          </span>
        ) : null}
      </button>

      {modelPath && !isPending ? (
        <button
          type="button"
          aria-label={t`Clear selected model`}
          className="text-muted-foreground hover:text-foreground flex size-6 shrink-0 items-center justify-center rounded-full"
          onClick={() => clearModel.mutate()}
        >
          <X className="size-3.5" />
        </button>
      ) : null}

      {modelPath && healthStatus === "pending" ? (
        <CircleNotch className="text-muted-foreground size-4 shrink-0 animate-spin" />
      ) : null}
      {modelPath && healthStatus === "success" ? (
        <Check className="size-4 shrink-0 text-green-600" />
      ) : null}
    </div>
  );
}

function formatModelSize(sizeBytes: number) {
  const unit = sizeBytes >= 1024 * 1024 * 1024 ? "GB" : "MB";
  const value =
    unit === "GB" ? sizeBytes / 1024 / 1024 / 1024 : sizeBytes / 1024 / 1024;
  return `${value.toLocaleString(undefined, {
    maximumFractionDigits: value >= 10 ? 0 : 1,
  })} ${unit}`;
}
