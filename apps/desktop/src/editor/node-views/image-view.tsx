import {
  type NodeViewComponentProps,
  useEditorEventCallback,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, useCallback, useRef, useState } from "react";

import { cn } from "@hypr/utils";

const MIN_IMAGE_WIDTH = 15;
const MAX_IMAGE_WIDTH = 100;
const DEFAULT_IMAGE_WIDTH = 80;

function clampImageWidth(value: number) {
  if (Number.isNaN(value)) return DEFAULT_IMAGE_WIDTH;
  return Math.min(
    MAX_IMAGE_WIDTH,
    Math.max(MIN_IMAGE_WIDTH, Math.round(value)),
  );
}

export function parseImageMetadata(title?: string) {
  const match = title?.match(/^char-editor-width=(\d{1,3})(?:\|(.*))?$/s);
  return {
    editorWidth:
      match && match.length >= 1
        ? clampImageWidth(parseInt(match[1], 10))
        : undefined,
    title: match && match.length >= 2 ? match[2] : title,
  };
}

export const imageNodeSpec: NodeSpec = {
  group: "block",
  draggable: true,
  attrs: {
    src: { default: null },
    alt: { default: null },
    title: { default: null },
    attachmentId: { default: null },
    editorWidth: { default: DEFAULT_IMAGE_WIDTH },
  },
  parseDOM: [
    {
      tag: "img[src]",
      getAttrs(dom) {
        const el = dom as HTMLElement;
        const title = el.getAttribute("title") ?? undefined;
        const metadata = parseImageMetadata(title);
        return {
          src: el.getAttribute("src"),
          alt: el.getAttribute("alt"),
          title: metadata.title,
          attachmentId: el.getAttribute("data-attachment-id"),
          editorWidth: clampImageWidth(
            parseInt(
              el.getAttribute("data-editor-width") ??
                String(metadata.editorWidth),
              10,
            ),
          ),
        };
      },
    },
  ],
  toDOM(node) {
    const attrs: Record<string, string> = {};
    if (node.attrs.src) attrs.src = node.attrs.src;
    if (node.attrs.alt) attrs.alt = node.attrs.alt;
    if (node.attrs.title) attrs.title = node.attrs.title;
    if (node.attrs.attachmentId) {
      attrs["data-attachment-id"] = node.attrs.attachmentId;
    }
    if (node.attrs.editorWidth) {
      attrs["data-editor-width"] = String(node.attrs.editorWidth);
    }
    return ["img", attrs];
  },
};

export const ResizableImageView = forwardRef<
  HTMLDivElement,
  NodeViewComponentProps
>(function ResizableImageView({ nodeProps, ...htmlAttrs }, ref) {
  const { node, getPos } = nodeProps;
  const [isHovered, setIsHovered] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftWidth, setDraftWidth] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const updateAttributes = useEditorEventCallback(
    (view, attrs: Record<string, unknown>) => {
      if (!view) return;
      const pos = getPos();
      const tr = view.state.tr.setNodeMarkup(pos, undefined, {
        ...node.attrs,
        ...attrs,
      });
      view.dispatch(tr);
    },
  );

  // to detect whether a nodeview is selected:
  // see: https://discuss.prosemirror.net/t/is-this-the-right-way-to-determine-if-a-nodeview-is-selected/2208/2
  // also: https://github.com/handlewithcarecollective/react-prosemirror/issues/161
  const pos = getPos();
  const { selection } = useEditorState();
  const isSelected =
    pos >= selection.from && pos + node.nodeSize <= selection.to;

  // we register all resize event handlers during resize start and unregister them on resize end.
  // all drag state lives inside this callback scope.
  // during a drag, draftWidth is a pixel value for immediate visual feedback.
  // once the drag ends, draftWidth resets to null and we calculate and persist the percentage as attributes.
  const handleResizeStart = useCallback(
    (
      direction: "left" | "right",
      event: React.PointerEvent<HTMLButtonElement>,
    ) => {
      const containerEl = containerRef.current;
      const imageEl = imageRef.current;
      if (!containerEl || !imageEl) return;

      event.preventDefault();
      event.stopPropagation();

      const editorEl = containerEl.closest(".ProseMirror");
      const maxWidth =
        editorEl?.getBoundingClientRect().width ??
        containerEl.getBoundingClientRect().width;
      const startWidth = imageEl.getBoundingClientRect().width;
      const startX = event.clientX;

      let currentWidth = startWidth;
      setIsResizing(true);
      setDraftWidth(startWidth);

      const handlePointerMove = (e: PointerEvent) => {
        const deltaX = (e.clientX - startX) * (direction === "left" ? -1 : 1);
        currentWidth = Math.min(maxWidth, Math.max(120, startWidth + deltaX));
        setDraftWidth(currentWidth);
      };

      const handlePointerUp = () => {
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);

        updateAttributes({
          editorWidth: clampImageWidth((currentWidth / maxWidth) * 100),
        });

        setIsResizing(false);
        setDraftWidth(null);
      };

      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp);
    },
    [updateAttributes],
  );

  const showControls = isHovered || isSelected || isResizing;
  const editorWidth = clampImageWidth(node.attrs.editorWidth);
  const imageWidth =
    draftWidth !== null ? `${draftWidth}px` : `${editorWidth}%`;

  return (
    <div
      ref={ref}
      {...htmlAttrs}
      className="relative overflow-visible select-none [&_*::selection]:bg-transparent [&::selection]:bg-transparent"
    >
      <div
        ref={containerRef}
        className="relative inline-block w-fit max-w-full overflow-visible"
        style={imageWidth ? { width: imageWidth } : undefined}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <img
          ref={imageRef}
          src={node.attrs.src}
          alt={node.attrs.alt || ""}
          title={parseImageMetadata(node.attrs.title).title ?? undefined}
          className={cn([
            "tiptap-image max-w-full rounded-md bg-white transition-[box-shadow,border-color] select-none",
            isSelected
              ? "ring-2 ring-blue-500 ring-offset-2 ring-offset-white"
              : "",
            isHovered && !isSelected
              ? "ring-1 ring-neutral-300 ring-offset-2 ring-offset-white"
              : "",
            "w-full",
          ])}
          draggable={false}
        />
        {showControls && (
          <>
            <div
              aria-hidden="true"
              className="absolute top-0 right-0 z-10 h-full w-6"
            />
            <div
              aria-hidden="true"
              className="absolute top-0 left-0 z-10 h-full w-6"
            />
            <button
              type="button"
              aria-label="Resize image from left"
              onPointerDown={(event) => handleResizeStart("left", event)}
              className="absolute top-1/2 left-1 z-20 flex h-14 w-4 -translate-y-1/2 cursor-ew-resize items-center justify-center rounded-full border border-neutral-300 bg-white/95 shadow-sm backdrop-blur-sm"
            >
              <span className="h-8 w-1 rounded-full bg-neutral-400" />
            </button>
            <button
              type="button"
              aria-label="Resize image from right"
              onPointerDown={(event) => handleResizeStart("right", event)}
              className="absolute top-1/2 right-1 z-20 flex h-14 w-4 -translate-y-1/2 cursor-ew-resize items-center justify-center rounded-full border border-neutral-300 bg-white/95 shadow-sm backdrop-blur-sm"
            >
              <span className="h-8 w-1 rounded-full bg-neutral-400" />
            </button>
          </>
        )}
      </div>
    </div>
  );
});
