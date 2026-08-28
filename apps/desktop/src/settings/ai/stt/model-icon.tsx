import { Apple } from "@lobehub/icons";
import * as stylex from "@stylexjs/stylex";
import type { ReactNode } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import type { StyleXProps } from "@anlg/ui/lib/stylex";

import { AiIconSlot, ProviderLobeIcon } from "~/settings/ai/shared";

type ModelIconSpec = {
  title: string;
  label?: string;
  sx?: StyleXProps["sx"];
  imageSrc?: string;
  imageSx?: StyleXProps["sx"];
  node?: ReactNode;
};

const MODEL_ICON_ASSET_BASE = "/assets/model-icons";
const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function getLocalModelIcon(model: string): ModelIconSpec | null {
  const value = model.toLowerCase();

  if (value === "cloud") {
    return {
      label: "A",
      title: "Anarlog Pro",
      imageSrc: ANARLOG_ICON_SRC,
    };
  }

  if (value === "apple-speech") {
    return {
      title: "Apple Speech",
      node: <ProviderLobeIcon icon={Apple} />,
    };
  }

  if (value.includes("qwen")) {
    return {
      label: "Q",
      title: "Qwen",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/qwen-logo.svg`,
    };
  }

  if (value.includes("omnilingual")) {
    return {
      label: "O",
      title: "Meta Omnilingual",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/meta-logo.svg`,
    };
  }

  if (value.includes("whisper") || value.includes("quantized")) {
    return {
      label: "W",
      title: "OpenAI Whisper",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/openai-logo.svg`,
    };
  }

  if (value.includes("parakeet")) {
    return {
      label: "P",
      title: "NVIDIA Parakeet",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/nvidia-logo.svg`,
      imageSx: styles.parakeetImage,
    };
  }

  if (value.includes("ggml") || value.includes("gguf")) {
    return {
      label: "G",
      title: "GGML",
      sx: modelIconVariantStyles.ggml,
    };
  }

  if (value.includes("soniqo")) {
    return {
      label: "S",
      title: "Soniqo",
      sx: modelIconVariantStyles.soniqo,
    };
  }

  return null;
}

export function getLocalModelBackendBadge(model: string): ModelIconSpec | null {
  const value = model.toLowerCase();

  if (value.includes("nvidia") || value.includes("cuda")) {
    return {
      label: "NV",
      title: "NVIDIA",
      sx: backendBadgeVariantStyles.nvidia,
    };
  }

  if (value === "apple-speech") {
    return null;
  }

  if (value.includes("apple") || value.includes("npu")) {
    return {
      label: "NPU",
      title: "Apple NPU",
      sx: backendBadgeVariantStyles.npu,
    };
  }

  if (value.includes("ggml") || value.includes("gguf")) {
    return {
      label: "GGML",
      title: "GGML runtime",
      sx: backendBadgeVariantStyles.ggml,
    };
  }

  return null;
}

export function LocalModelLabel({
  model,
  label,
  title,
  sx,
  labelSx,
}: {
  model: string;
  label: string;
  title?: string;
  labelSx?: StyleXProps["sx"];
} & StyleXProps) {
  const icon = getLocalModelIcon(model);

  return (
    <div title={title} {...stylex.props(styles.modelLabel, sx)}>
      {icon ? (
        <AiIconSlot title={icon.title} sx={icon.sx}>
          {icon.node ??
            (icon.imageSrc ? (
              <img
                src={icon.imageSrc}
                alt=""
                {...stylex.props(styles.modelImage, icon.imageSx)}
              />
            ) : (
              <span {...stylex.props(styles.initial)}>{icon.label}</span>
            ))}
        </AiIconSlot>
      ) : null}
      <span {...stylex.props(styles.label, labelSx)}>{label}</span>
    </div>
  );
}

export function LocalModelBackendBadge({ model }: { model: string }) {
  const badge = getLocalModelBackendBadge(model);

  if (!badge) {
    return null;
  }

  return (
    <span title={badge.title} {...stylex.props(styles.backendBadge, badge.sx)}>
      {badge.label}
    </span>
  );
}

const styles = stylex.create({
  backendBadge: {
    alignItems: "center",
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "inline-flex",
    flexShrink: 0,
    fontSize: "10px",
    fontWeight: 500,
    lineHeight: 1,
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  ggml: {
    backgroundColor: "rgb(255 251 235)",
    borderColor: "rgb(253 230 138)",
    borderRadius: radii.md,
    color: "rgb(180 83 9)",
  },
  initial: {
    fontSize: "10px",
    fontWeight: 600,
    lineHeight: 1,
  },
  label: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  modelImage: {
    objectFit: "contain",
    objectPosition: "center",
  },
  modelLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    minWidth: 0,
  },
  npu: {
    backgroundColor: colors.muted,
    borderColor: colors.border,
    color: colors.mutedForeground,
  },
  nvidia: {
    backgroundColor: "rgb(240 253 244)",
    borderColor: "rgb(187 247 208)",
    color: "rgb(21 128 61)",
  },
  parakeetImage: {
    objectFit: "cover",
    objectPosition: "left",
  },
  soniqo: {
    backgroundColor: "rgb(239 246 255)",
    borderColor: "rgb(191 219 254)",
    borderRadius: radii.md,
    color: "rgb(29 78 216)",
  },
});

const modelIconVariantStyles = {
  ggml: styles.ggml,
  soniqo: styles.soniqo,
} as const;

const backendBadgeVariantStyles = {
  ggml: styles.ggml,
  npu: styles.npu,
  nvidia: styles.nvidia,
} as const;
