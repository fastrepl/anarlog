import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import {
  ArrowSquareOut,
  File,
  FileText,
  FileXls,
  Image,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { commands as openerCommands } from "@anlg/plugin-opener2";

import {
  useAttachmentEditingEnabled,
  useAttachmentResolver,
} from "./attachment-resolver";
import { getSafeNodePos } from "./error-boundary";

const MIME_ICON_MAP: Record<string, typeof File> = {
  "application/pdf": FileText,
  "text/plain": FileText,
  "text/csv": FileXls,
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": FileXls,
  "application/vnd.ms-excel": FileXls,
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document":
    FileText,
  "application/msword": FileText,
};

function getFileIcon(mimeType: string) {
  if (mimeType.startsWith("image/")) return Image;
  return MIME_ICON_MAP[mimeType] ?? File;
}

function formatFileSize(bytes: number | null): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export const fileAttachmentNodeSpec: NodeSpec = {
  group: "block",
  draggable: true,
  atom: true,
  selectable: true,
  attrs: {
    attachmentId: { default: null },
    sharedAttachmentId: { default: null },
    name: { default: "" },
    mimeType: { default: "" },
    src: { default: null },
    path: { default: null },
    size: { default: null },
  },
  parseDOM: [
    {
      tag: 'div[data-type="file-attachment"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        return {
          attachmentId: el.getAttribute("data-attachment-id"),
          sharedAttachmentId: el.getAttribute("data-shared-attachment-id"),
          name: el.getAttribute("data-name"),
          mimeType: el.getAttribute("data-mime-type"),
          src: el.getAttribute("data-src"),
          size: el.getAttribute("data-size")
            ? Number(el.getAttribute("data-size"))
            : null,
        };
      },
    },
  ],
  toDOM(node) {
    const attrs: Record<string, string> = {
      "data-type": "file-attachment",
    };
    if (node.attrs.attachmentId) {
      attrs["data-attachment-id"] = node.attrs.attachmentId;
    }
    if (node.attrs.sharedAttachmentId) {
      attrs["data-shared-attachment-id"] = node.attrs.sharedAttachmentId;
    }
    if (node.attrs.name) attrs["data-name"] = node.attrs.name;
    if (node.attrs.mimeType) attrs["data-mime-type"] = node.attrs.mimeType;
    if (node.attrs.src) attrs["data-src"] = node.attrs.src;
    if (node.attrs.size != null) attrs["data-size"] = String(node.attrs.size);
    return ["div", attrs, node.attrs.name || "attachment"];
  },
};

export const FileAttachmentView = forwardRef<
  HTMLDivElement,
  NodeViewComponentProps
>(function FileAttachmentView({ nodeProps, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const resolveAttachment = useAttachmentResolver();
  const attachmentEditingEnabled = useAttachmentEditingEnabled();
  const [isHovered, setIsHovered] = useState(false);
  const attachmentId =
    typeof node.attrs.sharedAttachmentId === "string"
      ? node.attrs.sharedAttachmentId
      : node.attrs.attachmentId;
  const resolvedAttachment =
    typeof attachmentId === "string" ? resolveAttachment?.(attachmentId) : null;
  const { name, mimeType, size } = node.attrs;
  const src = resolvedAttachment?.src ?? node.attrs.src;
  const path = resolvedAttachment?.path ?? node.attrs.path;

  const Icon = getFileIcon(mimeType ?? "");
  const sizeLabel = formatFileSize(size);
  const displayName =
    name && name.length > 60 ? name.slice(0, 60) + "\u2026" : name || "file";

  const handleRemove = useEditorEventCallback((view) => {
    if (!view || !attachmentEditingEnabled || !view.editable) return;
    const pos = getSafeNodePos(getPos);
    if (pos === null) return;

    view.dispatch(view.state.tr.delete(pos, pos + node.nodeSize));
    view.focus();
  });

  const handleOpen = () => {
    if (path) {
      if (path.startsWith("https://")) {
        openerCommands.openUrl(path, null);
      } else {
        openerCommands.openPath(path, null);
      }
    }
  };

  const isImage = typeof mimeType === "string" && mimeType.startsWith("image/");

  return (
    <div ref={ref} {...htmlAttrs}>
      <div
        contentEditable={false}
        suppressContentEditableWarning
        {...stylex.props(styles.attachment)}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        {isImage && src ? (
          <img src={src} alt={name} {...stylex.props(styles.preview)} />
        ) : (
          <div {...stylex.props(styles.iconContainer)}>
            <Icon size={20} {...stylex.props(styles.mutedIcon)} />
          </div>
        )}

        <div {...stylex.props(styles.content)}>
          <div {...stylex.props(styles.fileName)}>{displayName}</div>
          {sizeLabel && (
            <div {...stylex.props(styles.fileSize)}>{sizeLabel}</div>
          )}
        </div>

        <div
          {...stylex.props(styles.actions, isHovered && styles.visibleActions)}
        >
          {src && (
            <button
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleOpen();
              }}
              {...stylex.props(styles.action)}
              title="Open file"
            >
              <ArrowSquareOut size={14} {...stylex.props(styles.mutedIcon)} />
            </button>
          )}
          {attachmentEditingEnabled ? (
            <button
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleRemove();
              }}
              {...stylex.props(styles.action)}
              title="Remove attachment"
            >
              <X size={14} {...stylex.props(styles.mutedIcon)} />
            </button>
          ) : null}
        </div>
      </div>
    </div>
  );
});

const styles = stylex.create({
  attachment: {
    alignItems: "center",
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.75rem",
    marginBlock: "0.25rem",
    paddingBlock: "0.625rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
  },
  preview: {
    borderRadius: "0.25rem",
    flexShrink: 0,
    height: "2.5rem",
    objectFit: "cover",
    width: "2.5rem",
  },
  iconContainer: {
    alignItems: "center",
    backgroundColor: `color-mix(in oklab, ${colors.accent} 60%, transparent)`,
    borderRadius: "0.25rem",
    display: "flex",
    flexShrink: 0,
    height: "2.5rem",
    justifyContent: "center",
    width: "2.5rem",
  },
  mutedIcon: {
    color: colors.mutedForeground,
  },
  content: {
    flexGrow: 1,
    minWidth: 0,
  },
  fileName: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  fileSize: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
    opacity: 0,
    transitionDuration: "150ms",
    transitionProperty: "opacity",
  },
  visibleActions: {
    opacity: 1,
  },
  action: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: "0.25rem",
    borderStyle: "none",
    padding: "0.25rem",
  },
});
