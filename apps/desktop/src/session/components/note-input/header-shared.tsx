import { json2md, parseJsonContent } from "@anlg/editor/markdown";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

export function getStoredNoteMarkdown(content: string | undefined) {
  const trimmed = content?.trim() ?? "";

  if (!trimmed) {
    return "";
  }

  if (!trimmed.startsWith("{")) {
    return trimmed;
  }

  return json2md(parseJsonContent(trimmed)).trim();
}

const UUID_TITLE_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const ISO_TITLE_RE = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/;

export function IconHeaderView({
  isActive,
  label,
  hoverLabel,
  icon,
  suffixIcon,
  pressed,
  onClick,
  onContextMenu,
  title,
  size = "tray",
  className,
}: {
  isActive: boolean;
  label: string;
  hoverLabel?: string;
  icon: React.ReactNode;
  suffixIcon?: React.ReactNode;
  pressed?: boolean;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  title?: string;
  size?: "tray" | "standalone";
  className?: string;
}) {
  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={label}
      aria-current={isActive ? "page" : undefined}
      aria-pressed={pressed}
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={title}
      data-hover-label={hoverLabel}
      className={iconHeaderViewClassName(
        isActive,
        size,
        cn([
          "px-2",
          isActive
            ? suffixIcon
              ? "max-w-40 min-w-10 gap-1.5 @max-[480px]:max-w-12 @max-[480px]:min-w-12 @max-[480px]:gap-0 @max-[480px]:px-1.5"
              : "max-w-40 min-w-10 gap-1.5 @max-[480px]:max-w-10 @max-[480px]:gap-0"
            : null,
          hoverLabel
            ? "after:hidden after:min-w-0 after:truncate after:text-xs after:font-medium after:content-[attr(data-hover-label)] hover:after:block"
            : null,
          className,
        ]),
      )}
    >
      {icon}
      {isActive && (
        <span
          className={cn([
            "min-w-0 truncate text-xs font-medium @max-[480px]:sr-only",
            hoverLabel ? "group-hover/header-view:hidden" : null,
          ])}
        >
          {label}
        </span>
      )}
      {isActive ? suffixIcon : null}
    </button>
  );
}

export function iconHeaderViewClassName(
  isActive: boolean,
  size: "tray" | "standalone" = "tray",
  className?: string,
) {
  const heightClassName = size === "tray" ? "h-[26px]" : "h-7";

  return cn([
    "group/header-view rounded-pill flex shrink-0 items-center justify-center transition-colors select-none [corner-shape:round] [&>svg]:shrink-0",
    isActive
      ? [
          "text-foreground bg-white shadow-xs",
          "dark:bg-accent dark:text-foreground dark:shadow-none",
        ]
      : [
          "text-muted-foreground/70",
          "hover:bg-background/60 hover:text-foreground",
          "dark:hover:bg-accent/80 dark:hover:text-foreground",
        ],
    heightClassName,
    className,
  ]);
}

export function getEnhancedNoteTitle({
  rawTitle,
  templateTitle,
  templateId,
}: {
  rawTitle: unknown;
  templateTitle: string | null;
  templateId: string | undefined;
}) {
  if (templateTitle) {
    return templateTitle;
  }

  const title = typeof rawTitle === "string" ? rawTitle.trim() : "";
  if (!title) {
    return "Summary";
  }

  const isGeneratedTitle =
    title === "Summary" ||
    title === templateId ||
    UUID_TITLE_RE.test(title) ||
    ISO_TITLE_RE.test(title);

  if (isGeneratedTitle) {
    return "Summary";
  }

  return title;
}

export async function copyTextToClipboard(
  text: string,
  messages?: {
    success: string;
    error: string;
  },
) {
  try {
    try {
      await navigator.clipboard.write([
        new ClipboardItem({
          "text/plain": new Blob([text], {
            type: "text/plain",
          }),
          "text/markdown": new Blob([text], {
            type: "text/markdown",
          }),
        }),
      ]);
    } catch {
      // Fallback for environments that do not support text/markdown
      await navigator.clipboard.writeText(text);
    }

    if (messages) {
      sonnerToast.success(messages.success);
    }

    return true;
  } catch (error) {
    console.error("Failed to copy note content", error);

    if (messages) {
      sonnerToast.error(messages.error);
    }

    return false;
  }
}
