import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { File, X } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

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
        {...stylex.props(styles.attachment)}
      >
        {isImage && url ? (
          <img src={url} alt={name} {...stylex.props(styles.preview)} />
        ) : (
          <File size={12} {...stylex.props(styles.icon)} />
        )}
        <span {...stylex.props(styles.name)}>{displayName}</span>
        <button
          type="button"
          onMouseDown={(e) => {
            e.preventDefault();
            e.stopPropagation();
            handleRemove();
          }}
          {...stylex.props(styles.remove)}
        >
          <X size={10} />
        </button>
      </span>
    </span>
  );
});

const styles = stylex.create({
  attachment: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderColor: colors.border,
    borderRadius: radii.md,
    borderStyle: "solid",
    borderWidth: "1px",
    color: colors.mutedForeground,
    display: "inline-flex",
    fontSize: "0.75rem",
    gap: "0.25rem",
    lineHeight: "1rem",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
  },
  preview: {
    borderRadius: "0.25rem",
    flexShrink: 0,
    height: "1rem",
    objectFit: "cover",
    width: "1rem",
  },
  icon: {
    color: colors.mutedForeground,
    flexShrink: 0,
  },
  name: {
    maxWidth: "120px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  remove: {
    backgroundColor: {
      default: "transparent",
      ":hover": colors.accent,
    },
    borderRadius: "0.25rem",
    borderStyle: "none",
    flexShrink: 0,
    padding: "0.125rem",
  },
});
