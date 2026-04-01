import { NodeViewWrapper, ReactNodeViewRenderer } from "@tiptap/react";
import type { NodeViewProps } from "@tiptap/react";
import { useCallback, useEffect, useRef, useState } from "react";

import {
  AttachmentImage,
  DEFAULT_EDITOR_WIDTH,
  normalizeEditorWidth,
} from "@hypr/tiptap/shared";
import { cn } from "@hypr/utils";

function ImageNodeView({ node, updateAttributes, selected }: NodeViewProps) {
  const [isHovered, setIsHovered] = useState(false);
  const [isFocused, setIsFocused] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftWidth, setDraftWidth] = useState<number | null>(null);
  const [altText, setAltText] = useState(node.attrs.alt || "");
  const inputRef = useRef<HTMLInputElement>(null);
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
    setAltText(node.attrs.alt || "");
  }, [node.attrs.alt]);

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

  const handleAltChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const newAlt = e.target.value;
      setAltText(newAlt);
      updateAttributes({ alt: newAlt });
    },
    [updateAttributes],
  );

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      inputRef.current?.blur();
    }
  }, []);

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

  const showControls = isHovered || isFocused || selected || isResizing;
  const editorWidth =
    normalizeEditorWidth(node.attrs.editorWidth) ?? DEFAULT_EDITOR_WIDTH;
  const imageWidth =
    draftWidth !== null ? `${draftWidth}px` : `${editorWidth}%`;

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
          title={node.attrs.title || undefined}
          className={cn(["tiptap-image max-w-full", "w-full"])}
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
        {showControls && (
          <div className="absolute right-2 bottom-2 left-2 rounded-md border border-neutral-200 bg-white/95 p-2 shadow-lg backdrop-blur-sm">
            <label className="flex items-center gap-2">
              <span className="text-xs whitespace-nowrap text-neutral-500">
                Alt text:
              </span>
              <input
                ref={inputRef}
                type="text"
                value={altText}
                onChange={handleAltChange}
                onKeyDown={handleKeyDown}
                onFocus={() => setIsFocused(true)}
                onBlur={() => setIsFocused(false)}
                placeholder="Describe this image..."
                className="flex-1 border-none bg-transparent text-sm text-neutral-700 outline-none placeholder:text-neutral-400"
              />
            </label>
          </div>
        )}
      </div>
    </NodeViewWrapper>
  );
}

export const BlogImage = AttachmentImage.extend({
  addNodeView() {
    return ReactNodeViewRenderer(ImageNodeView);
  },
});
