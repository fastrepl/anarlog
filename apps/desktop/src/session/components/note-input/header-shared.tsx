import * as stylex from "@stylexjs/stylex";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { json2md, parseJsonContent } from "@anlg/editor/markdown";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

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
  onClick,
  onContextMenu,
  title,
  size = "tray",
  sx,
}: {
  isActive: boolean;
  label: string;
  hoverLabel?: string;
  icon: React.ReactNode;
  onClick?: () => void;
  onContextMenu?: React.MouseEventHandler<HTMLButtonElement>;
  title?: string;
  size?: "tray" | "standalone";
  sx?: stylex.StyleXStyles;
}) {
  return (
    <button
      data-main-area-window-drag-region
      data-tauri-drag-region="false"
      type="button"
      aria-label={label}
      aria-current={isActive ? "page" : undefined}
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={title}
      data-hover-label={hoverLabel}
      data-header-view
      {...stylex.props(
        getIconHeaderViewStyles(isActive, size, sx),
        styles.horizontalPadding,
        isActive && styles.activeContent,
      )}
    >
      <span {...stylex.props(styles.iconSlot)}>{icon}</span>
      {isActive && (
        <span
          {...stylex.props(
            styles.label,
            Boolean(hoverLabel) && styles.hideLabelOnHover,
          )}
        >
          {label}
        </span>
      )}
      {hoverLabel ? (
        <span {...stylex.props(styles.hoverLabel)}>{hoverLabel}</span>
      ) : null}
    </button>
  );
}

export function getIconHeaderViewStyles(
  isActive: boolean,
  size: "tray" | "standalone" = "tray",
  sx?: stylex.StyleXStyles,
) {
  return [
    styles.headerView,
    isActive ? styles.active : styles.inactive,
    size === "tray" ? styles.tray : styles.standalone,
    sx,
  ];
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

const compact = "@container (max-width: 480px)";

const styles = stylex.create({
  active: {
    backgroundColor: {
      default: "white",
      ":is(.dark *)": colors.accent,
    },
    boxShadow: {
      default: shadows.sm,
      ":is(.dark *)": "none",
    },
    color: colors.foreground,
  },
  activeContent: {
    gap: {
      default: "0.375rem",
      [compact]: 0,
    },
    maxWidth: {
      default: "10rem",
      [compact]: "2.5rem",
    },
    minWidth: "2.5rem",
  },
  headerView: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "flex",
    flexShrink: 0,
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    userSelect: "none",
  },
  hideLabelOnHover: {
    display: {
      default: null,
      ":is([data-header-view]:hover *)": "none",
    },
  },
  horizontalPadding: {
    paddingInline: "0.5rem",
  },
  hoverLabel: {
    display: {
      default: "none",
      ":is([data-header-view]:hover *)": "block",
    },
    fontSize: "0.75rem",
    fontWeight: 500,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  iconSlot: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    justifyContent: "center",
  },
  inactive: {
    backgroundColor: {
      default: null,
      ":hover": `color-mix(in oklab, ${colors.background} 60%, transparent)`,
      ":is(.dark *):hover": `color-mix(in oklab, ${colors.accent} 80%, transparent)`,
    },
    color: {
      default: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
      ":hover": colors.foreground,
    },
  },
  label: {
    clip: {
      default: null,
      [compact]: "rect(0, 0, 0, 0)",
    },
    fontSize: "0.75rem",
    fontWeight: 500,
    height: {
      default: null,
      [compact]: "1px",
    },
    margin: {
      default: null,
      [compact]: "-1px",
    },
    minWidth: 0,
    overflow: "hidden",
    padding: {
      default: null,
      [compact]: 0,
    },
    position: {
      default: null,
      [compact]: "absolute",
    },
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    width: {
      default: null,
      [compact]: "1px",
    },
  },
  standalone: {
    height: "1.75rem",
  },
  tray: {
    height: "26px",
  },
});

export { styles as headerViewStyles };
