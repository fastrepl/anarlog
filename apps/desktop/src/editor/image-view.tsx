import {
  type NodeViewComponentProps,
  useEditorEventCallback,
} from "@handlewithcare/react-prosemirror";
import { AllSelection, NodeSelection } from "prosemirror-state";
import { forwardRef, useCallback, useEffect, useRef, useState } from "react";

import {
  DEFAULT_EDITOR_WIDTH,
  normalizeEditorWidth,
  stripEditorWidthFromTitle,
} from "@hypr/tiptap/shared";
import { cn } from "@hypr/utils";

export const ResizableImageView = forwardRef<
  HTMLElement,
  NodeViewComponentProps
>(({ nodeProps, ...htmlAttrs }, ref) => {
  const { node, getPos } = nodeProps;
  const [isHovered, setIsHovered] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [isRangeSelected, setIsRangeSelected] = useState(false);
  const [isAllSelected, setIsAllSelected] = useState(false);
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

  const checkSelection = useEditorEventCallback((view) => {
    if (!view) return;
    const pos = getPos();
    const { doc, selection } = view.state;
    const nodeStart = pos;
    const nodeEnd = pos + node.nodeSize;
    const isNodeSel =
      selection instanceof NodeSelection && selection.from === nodeStart;
    const includesNode =
      !selection.empty &&
      !isNodeSel &&
      selection.from <= nodeStart &&
      selection.to >= nodeEnd;

    setIsRangeSelected(includesNode);
    setIsAllSelected(
      selection instanceof AllSelection ||
        (selection.from <= 1 && selection.to >= doc.content.size - 1),
    );
  });

  useEffect(() => {
    checkSelection();
  });

  useEffect(() => {
    if (!isResizing) return;

    const handlePointerMove = (event: PointerEvent) => {
      const resizeState = resizeStateRef.current;
      if (!resizeState) return;

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
      if (!container || !image) return;

      event.preventDefault();
      event.stopPropagation();

      const editorElement = container.closest(".ProseMirror");
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

  const selected = nodeProps.decorations.some(
    (d) => (d as any).type?.name === "selected",
  );

  const isSelected = selected || isRangeSelected;
  const showControls = !isAllSelected && (isHovered || selected || isResizing);
  const editorWidth =
    normalizeEditorWidth(node.attrs.editorWidth) ?? DEFAULT_EDITOR_WIDTH;
  const imageWidth =
    draftWidth !== null ? `${draftWidth}px` : `${editorWidth}%`;

  return (
    <div
      ref={ref as any}
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
          title={stripEditorWidthFromTitle(node.attrs.title)}
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

ResizableImageView.displayName = "ResizableImageView";
