import Image from "@tiptap/extension-image";
import { NodeViewWrapper, ReactNodeViewRenderer } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import {
  normalizeEditorWidth,
  parseImageTitleMetadata,
  serializeImageTitleMetadata,
  stripEditorWidthFromTitle,
} from "./image-metadata";

function ResizableImageNodeView({
  node,
  updateAttributes,
  selected,
  editor,
}: NodeViewProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftWidth, setDraftWidth] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const draftWidthRef = useRef<number | null>(null);
  const resizeStateRef = useRef<{
    direction: "left" | "right";
    editorWidth: number;
    startWidth: number;
    startX: number;
  } | null>(null);

  useEffect(() => {
    if (!isResizing) {
      return;
    }

    const handlePointerMove = (event: PointerEvent) => {
      const resizeState = resizeStateRef.current;
      if (!resizeState) {
        return;
      }

      const deltaX =
        (event.clientX - resizeState.startX) *
        (resizeState.direction === "left" ? -1 : 1);
      const nextWidth = Math.min(
        resizeState.editorWidth,
        Math.max(120, resizeState.startWidth + deltaX),
      );

      draftWidthRef.current = nextWidth;
      setDraftWidth(nextWidth);
    };

    const handlePointerUp = () => {
      const resizeState = resizeStateRef.current;
      if (!resizeState || !draftWidthRef.current) {
        resizeStateRef.current = null;
        draftWidthRef.current = null;
        setIsResizing(false);
        setDraftWidth(null);
        return;
      }

      updateAttributes({
        editorWidth: normalizeEditorWidth(
          (draftWidthRef.current / resizeState.editorWidth) * 100,
        ),
      });

      resizeStateRef.current = null;
      draftWidthRef.current = null;
      setIsResizing(false);
      setDraftWidth(null);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    return () => {
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
  }, [isResizing, updateAttributes]);

  const handleResizeStart = useCallback(
    (
      direction: "left" | "right",
      event: React.PointerEvent<HTMLButtonElement>,
    ) => {
      const container = containerRef.current;
      const image = imageRef.current;
      if (!container || !image) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const editorElement = container.closest(".tiptap");
      const editorWidth =
        editorElement?.getBoundingClientRect().width ??
        container.getBoundingClientRect().width;

      resizeStateRef.current = {
        direction,
        editorWidth,
        startWidth: image.getBoundingClientRect().width,
        startX: event.clientX,
      };

      draftWidthRef.current = image.getBoundingClientRect().width;
      setIsResizing(true);
      setDraftWidth(image.getBoundingClientRect().width);
    },
    [],
  );

  const showControls =
    editor.isEditable && (isHovered || selected || isResizing);
  const imageWidth =
    draftWidth !== null
      ? `${draftWidth}px`
      : node.attrs.editorWidth
        ? `${node.attrs.editorWidth}%`
        : undefined;
  const hasExplicitWidth = imageWidth !== undefined;

  return (
    <NodeViewWrapper className="relative overflow-visible">
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
          title={stripEditorWidthFromTitle(node.attrs.title)}
          className={cn([
            "tiptap-image max-w-full",
            selected ? "rounded-md bg-white ring-1 ring-neutral-200" : "",
            hasExplicitWidth ? "w-full" : "",
          ])}
          draggable={false}
        />
        {showControls && (
          <>
            <div
              aria-hidden="true"
              className="absolute top-0 right-full z-10 h-full w-3"
            />
            <div
              aria-hidden="true"
              className="absolute top-0 left-full z-10 h-full w-3"
            />
            <button
              type="button"
              aria-label="Resize image from left"
              onPointerDown={(event) => handleResizeStart("left", event)}
              className="absolute top-1/2 right-full z-20 mr-3 flex h-16 w-4 -translate-y-1/2 cursor-ew-resize items-center justify-center rounded-full border border-neutral-300 bg-white/95 shadow-sm backdrop-blur-sm"
            >
              <span className="h-8 w-1 rounded-full bg-neutral-400" />
            </button>
            <button
              type="button"
              aria-label="Resize image from right"
              onPointerDown={(event) => handleResizeStart("right", event)}
              className="absolute top-1/2 left-full z-20 ml-3 flex h-16 w-4 -translate-y-1/2 cursor-ew-resize items-center justify-center rounded-full border border-neutral-300 bg-white/95 shadow-sm backdrop-blur-sm"
            >
              <span className="h-8 w-1 rounded-full bg-neutral-400" />
            </button>
          </>
        )}
      </div>
    </NodeViewWrapper>
  );
}

export const AttachmentImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      title: {
        default: null,
        parseHTML: (element) =>
          stripEditorWidthFromTitle(element.getAttribute("title")) ?? null,
        renderHTML: (attributes) => {
          if (!attributes.title) {
            return {};
          }

          return { title: attributes.title };
        },
      },
      attachmentId: {
        default: null,
        parseHTML: (element) => element.getAttribute("data-attachment-id"),
        renderHTML: (attributes) => {
          if (!attributes.attachmentId) {
            return {};
          }
          return { "data-attachment-id": attributes.attachmentId };
        },
      },
      editorWidth: {
        default: null,
        parseHTML: (element) => {
          const attr = element.getAttribute("data-editor-width");
          if (attr) {
            return normalizeEditorWidth(Number(attr));
          }

          return parseImageTitleMetadata(element.getAttribute("title"))
            .editorWidth;
        },
        renderHTML: (attributes) => {
          const editorWidth = normalizeEditorWidth(attributes.editorWidth);
          if (!editorWidth) {
            return {};
          }

          return { "data-editor-width": editorWidth };
        },
      },
    };
  },

  addNodeView() {
    return ReactNodeViewRenderer(ResizableImageNodeView);
  },

  parseMarkdown: (token: { href?: string; text?: string; title?: string }) => {
    const metadata = parseImageTitleMetadata(token.title);
    const src = token.href || "";

    return {
      type: "image",
      attrs: {
        src,
        alt: token.text || "",
        title: metadata.title,
        attachmentId: null,
        editorWidth: metadata.editorWidth,
      },
    };
  },

  renderMarkdown: (node: {
    attrs?: {
      src?: string;
      alt?: string;
      title?: string;
      editorWidth?: number | null;
    };
  }) => {
    const src = node.attrs?.src || "";
    const alt = node.attrs?.alt || "";
    const title = serializeImageTitleMetadata({
      editorWidth: node.attrs?.editorWidth,
      title: node.attrs?.title,
    });

    return title ? `![${alt}](${src} "${title}")` : `![${alt}](${src})`;
  },
});
