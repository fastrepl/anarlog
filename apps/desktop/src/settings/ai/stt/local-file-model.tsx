import { Trans, useLingui } from "@lingui/react/macro";
import { Check, CircleNotch, FolderOpen, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open as selectFile } from "@tauri-apps/plugin-dialog";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
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
    <div {...stylex.props(styles.control)}>
      <button
        type="button"
        {...stylex.props(styles.chooseButton)}
        title={modelPath || undefined}
        disabled={isPending}
        onClick={() => chooseModel.mutate()}
      >
        {chooseModel.isPending ? (
          <CircleNotch {...stylex.props(styles.spinner)} />
        ) : (
          <FolderOpen {...stylex.props(styles.icon)} />
        )}
        <span {...stylex.props(styles.filename)}>
          {filename || <Trans>Choose model file</Trans>}
        </span>
        {modelInfo.data ? (
          <span {...stylex.props(styles.metadata)}>
            {formatModelSize(modelInfo.data.sizeBytes)} · GGML ·{" "}
            <Trans>After recording</Trans>
          </span>
        ) : null}
      </button>

      {modelPath && !isPending ? (
        <button
          type="button"
          aria-label={t`Clear selected model`}
          {...stylex.props(styles.clearButton)}
          onClick={() => clearModel.mutate()}
        >
          <X {...stylex.props(styles.clearIcon)} />
        </button>
      ) : null}

      {modelPath && healthStatus === "pending" ? (
        <CircleNotch {...stylex.props(styles.statusSpinner)} />
      ) : null}
      {modelPath && healthStatus === "success" ? (
        <Check {...stylex.props(styles.success)} />
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

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  chooseButton: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.5rem",
    minWidth: 0,
    textAlign: "left",
  },
  clearButton: {
    alignItems: "center",
    borderRadius: radii.full,
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    display: "flex",
    flexShrink: 0,
    height: "1.5rem",
    justifyContent: "center",
    width: "1.5rem",
  },
  clearIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  control: {
    alignItems: "center",
    backgroundColor: colors.card,
    borderColor: colors.input,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "none",
    display: "flex",
    gap: "0.5rem",
    height: "2.25rem",
    minWidth: 0,
    paddingInline: "0.75rem",
    textAlign: "left",
  },
  filename: {
    flex: "1",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  icon: {
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  metadata: {
    color: colors.mutedForeground,
    flexShrink: 0,
    fontSize: "11px",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  statusSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
  success: {
    color: "rgb(22 163 74)",
    flexShrink: 0,
    height: "1rem",
    width: "1rem",
  },
});
