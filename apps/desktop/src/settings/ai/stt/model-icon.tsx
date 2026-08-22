import { Apple } from "@lobehub/icons";
import type { ReactNode } from "react";

import { cn } from "@anlg/utils";

import { AiIconSlot, ProviderLobeIcon } from "~/settings/ai/shared";

type ModelIconSpec = {
  title: string;
  label?: string;
  className?: string;
  imageSrc?: string;
  imageClassName?: string;
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
      imageClassName: "object-cover object-left",
    };
  }

  if (value.includes("ggml") || value.includes("gguf")) {
    return {
      label: "G",
      title: "GGML",
      className: "border-amber-200 bg-amber-50 text-amber-700",
    };
  }

  if (value.includes("soniqo")) {
    return {
      label: "S",
      title: "Soniqo",
      className: "border-blue-200 bg-blue-50 text-blue-700",
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
      className: "border-green-200 bg-green-50 text-green-700",
    };
  }

  if (value === "apple-speech") {
    return null;
  }

  if (value.includes("apple") || value.includes("npu")) {
    return {
      label: "NPU",
      title: "Apple NPU",
      className: "border-border bg-muted text-muted-foreground",
    };
  }

  if (value.includes("ggml") || value.includes("gguf")) {
    return {
      label: "GGML",
      title: "GGML runtime",
      className: "border-amber-200 bg-amber-50 text-amber-700",
    };
  }

  return null;
}

export function LocalModelLabel({
  model,
  label,
  title,
  className,
  labelClassName,
}: {
  model: string;
  label: string;
  title?: string;
  className?: string;
  labelClassName?: string;
}) {
  const icon = getLocalModelIcon(model);

  return (
    <div
      title={title}
      className={cn(["flex min-w-0 items-center gap-2", className])}
    >
      {icon ? (
        <AiIconSlot title={icon.title} className={icon.className}>
          {icon.node ??
            (icon.imageSrc ? (
              <img
                src={icon.imageSrc}
                alt=""
                className={cn([
                  "object-contain object-center",
                  icon.imageClassName,
                ])}
              />
            ) : (
              <span className="text-[10px] leading-none font-semibold">
                {icon.label}
              </span>
            ))}
        </AiIconSlot>
      ) : null}
      <span className={cn(["min-w-0 truncate", labelClassName])}>{label}</span>
    </div>
  );
}

export function LocalModelBackendBadge({ model }: { model: string }) {
  const badge = getLocalModelBackendBadge(model);

  if (!badge) {
    return null;
  }

  return (
    <span
      title={badge.title}
      className={cn([
        "inline-flex shrink-0 items-center rounded-md border px-1.5 py-0.5 text-[10px] leading-none font-medium",
        badge.className,
      ])}
    >
      {badge.label}
    </span>
  );
}
