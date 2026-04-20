import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import {
  ExternalLinkIcon,
  FileIcon,
  FileSpreadsheetIcon,
  FileTextIcon,
  ImageIcon,
  XIcon,
} from "lucide-react";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef } from "react";

import { commands as openerCommands } from "@hypr/plugin-opener2";
import { cn } from "@hypr/utils";

const MIME_ICON_MAP: Record<string, typeof FileIcon> = {
  "application/pdf": FileTextIcon,
  "text/plain": FileTextIcon,
  "text/csv": FileSpreadsheetIcon,
  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet":
    FileSpreadsheetIcon,
  "application/vnd.ms-excel": FileSpreadsheetIcon,
  "application/vnd.openxmlformats-officedocument.wordprocessingml.document":
    FileTextIcon,
  "application/msword": FileTextIcon,
};

function getFileIcon(mimeType: string) {
  if (mimeType.startsWith("image/")) return ImageIcon;
  return MIME_ICON_MAP[mimeType] ?? FileIcon;
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
  const { name, mimeType, src, path, size } = node.attrs;

  const Icon = getFileIcon(mimeType ?? "");
  const sizeLabel = formatFileSize(size);
  const displayName =
    name && name.length > 60 ? name.slice(0, 60) + "\u2026" : name || "file";

  const handleRemove = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getPos();
    view.dispatch(view.state.tr.delete(pos, pos + node.nodeSize));
    view.focus();
  });

  const handleOpen = () => {
    if (path) {
      openerCommands.openPath(path, null);
    }
  };

  const isImage = typeof mimeType === "string" && mimeType.startsWith("image/");

  return (
    <div ref={ref} {...htmlAttrs}>
      <div
        contentEditable={false}
        suppressContentEditableWarning
        className={cn([
          "group my-1 flex items-center gap-3 rounded-lg border border-neutral-200 bg-neutral-50 px-3 py-2.5",
          "hover:border-neutral-300 hover:bg-neutral-100",
          "transition-colors",
        ])}
      >
        {isImage && src ? (
          <img
            src={src}
            alt={name}
            className="h-10 w-10 shrink-0 rounded object-cover"
          />
        ) : (
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded bg-neutral-200/60">
            <Icon size={20} className="text-neutral-500" />
          </div>
        )}

        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-medium text-neutral-700">
            {displayName}
          </div>
          {sizeLabel && (
            <div className="text-xs text-neutral-400">{sizeLabel}</div>
          )}
        </div>

        <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          {src && (
            <button
              type="button"
              onMouseDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                handleOpen();
              }}
              className="rounded p-1 hover:bg-neutral-200"
              title="Open file"
            >
              <ExternalLinkIcon size={14} className="text-neutral-500" />
            </button>
          )}
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault();
              e.stopPropagation();
              handleRemove();
            }}
            className="rounded p-1 hover:bg-neutral-200"
            title="Remove attachment"
          >
            <XIcon size={14} className="text-neutral-500" />
          </button>
        </div>
      </div>
    </div>
  );
});
