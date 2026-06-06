import { cn } from "@hypr/utils";

type ModelIconSpec = {
  label: string;
  title: string;
  className: string;
  imageSrc?: string;
  imageClassName?: string;
};

const MODEL_ICON_ASSET_BASE = "/assets/model-icons";
const ANARLOG_ICON_SRC = "/assets/anarlog-icon.png";

export function getLocalModelIcon(model: string): ModelIconSpec | null {
  const value = model.toLowerCase();

  if (value === "cloud") {
    return {
      label: "A",
      title: "Anarlog Pro",
      className: "border-border bg-card text-muted-foreground",
      imageSrc: ANARLOG_ICON_SRC,
      imageClassName: "size-4 object-contain",
    };
  }

  if (value.includes("qwen")) {
    return {
      label: "Q",
      title: "Qwen",
      className: "border-border bg-card text-muted-foreground",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/qwen-logo.svg`,
      imageClassName: "size-4 object-contain",
    };
  }

  if (value.includes("omnilingual")) {
    return {
      label: "O",
      title: "Meta Omnilingual",
      className: "border-border bg-card text-muted-foreground",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/meta-logo.svg`,
      imageClassName: "size-4 object-contain",
    };
  }

  if (value.includes("whisper") || value.includes("quantized")) {
    return {
      label: "W",
      title: "OpenAI Whisper",
      className: "border-border bg-card text-muted-foreground",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/openai-logo.svg`,
      imageClassName: "size-4 object-contain",
    };
  }

  if (value.includes("parakeet")) {
    return {
      label: "P",
      title: "NVIDIA Parakeet",
      className: "border-border bg-card text-muted-foreground",
      imageSrc: `${MODEL_ICON_ASSET_BASE}/nvidia-logo.svg`,
      imageClassName: "size-4 object-cover object-left",
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
  className,
  labelClassName,
}: {
  model: string;
  label: string;
  className?: string;
  labelClassName?: string;
}) {
  const icon = getLocalModelIcon(model);

  return (
    <div className={cn(["flex min-w-0 items-center gap-2", className])}>
      {icon?.imageSrc ? (
        <img
          title={icon.title}
          aria-label={icon.title}
          src={icon.imageSrc}
          alt=""
          className={cn([
            "shrink-0 object-contain object-center",
            icon.imageClassName,
            "size-5",
          ])}
        />
      ) : icon ? (
        <span
          title={icon.title}
          aria-label={icon.title}
          className={cn([
            "inline-flex size-5 shrink-0 items-center justify-center overflow-hidden rounded-md text-[10px] leading-none font-semibold",
            icon.className,
          ])}
        >
          {icon.label}
        </span>
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
