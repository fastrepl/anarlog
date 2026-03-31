import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { FileIcon, XIcon } from "lucide-react";
import { forwardRef } from "react";

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
    const pos = getPos();
    view.dispatch(view.state.tr.delete(pos, pos + node.nodeSize));
    view.focus();
  });

  return (
    <span ref={ref as any} {...htmlAttrs}>
      <span
        contentEditable={false}
        suppressContentEditableWarning
        className="inline-flex items-center gap-1 rounded-md border border-neutral-200 bg-neutral-50 px-1.5 py-0.5 text-xs text-neutral-600"
      >
        {isImage && url ? (
          <img
            src={url}
            alt={name}
            className="h-4 w-4 shrink-0 rounded object-cover"
          />
        ) : (
          <FileIcon size={12} className="shrink-0 text-neutral-400" />
        )}
        <span className="max-w-[120px] truncate">{displayName}</span>
        <button
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleRemove();
          }}
          className="shrink-0 rounded p-0.5 hover:bg-neutral-200"
        >
          <XIcon size={10} />
        </button>
      </span>
    </span>
  );
});
