import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { FileIcon, XIcon } from "lucide-react";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef } from "react";

import { getSafeNodePos } from "./error-boundary";

export const attachmentNodeSpec: NodeSpec = {
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  attrs: {
    id: { default: null },
    name: { default: "" },
    mimeType: { default: "" },
    url: { default: null },
    size: { default: null },
  },
  parseDOM: [
    {
      tag: 'span[data-type="attachment"]',
      getAttrs(dom) {
        const el = dom as HTMLElement;
        return {
          id: el.getAttribute("data-id"),
          name: el.getAttribute("data-name"),
          mimeType: el.getAttribute("data-mime-type"),
          url: el.getAttribute("data-url"),
          size: el.getAttribute("data-size")
            ? Number(el.getAttribute("data-size"))
            : null,
        };
      },
    },
  ],
  toDOM(node) {
    const attrs: Record<string, string> = { "data-type": "attachment" };
    if (node.attrs.id) attrs["data-id"] = node.attrs.id;
    if (node.attrs.name) attrs["data-name"] = node.attrs.name;
    if (node.attrs.mimeType) attrs["data-mime-type"] = node.attrs.mimeType;
    if (node.attrs.url) attrs["data-url"] = node.attrs.url;
    if (node.attrs.size != null) attrs["data-size"] = String(node.attrs.size);
    return ["span", attrs, node.attrs.name || "attachment"];
  },
};

export const AttachmentChipView = forwardRef<
  HTMLSpanElement,
  NodeViewComponentProps
>(function AttachmentChipView({ nodeProps, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const { name, mimeType, url } = node.attrs;
  const isImage = typeof mimeType === "string" && mimeType.startsWith("image/");
  const displayName =
    name && name.length > 24 ? name.slice(0, 24) + "\u2026" : name || "file";

  const handleRemove = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getSafeNodePos(getPos);
    if (pos === null) return;

    view.dispatch(view.state.tr.delete(pos, pos + node.nodeSize));
    view.focus();
  });

  return (
    <span ref={ref as any} {...htmlAttrs}>
      <span
        contentEditable={false}
        suppressContentEditableWarning
        className="border-border bg-muted text-muted-foreground inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-xs"
      >
        {isImage && url ? (
          <img
            src={url}
            alt={name}
            className="h-4 w-4 shrink-0 rounded object-cover"
          />
        ) : (
          <FileIcon size={12} className="text-muted-foreground shrink-0" />
        )}
        <span className="max-w-[120px] truncate">{displayName}</span>
        <button
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleRemove();
          }}
          className="hover:bg-accent shrink-0 rounded p-0.5"
        >
          <XIcon size={10} />
        </button>
      </span>
    </span>
  );
});
