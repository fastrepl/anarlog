import {
  type NodeViewComponentProps,
  useEditorEventCallback,
  useIsNodeSelected,
} from "@handlewithcare/react-prosemirror";
import * as stylex from "@stylexjs/stylex";
import type { NodeSpec } from "prosemirror-model";
import { forwardRef, useCallback, useRef, useState } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { cn } from "@anlg/utils";

import {
  useAttachmentEditingEnabled,
  useAttachmentResolver,
} from "./attachment-resolver";
import { getSafeNodePos } from "./error-boundary";

const MIN_IMAGE_WIDTH = 15;
const MAX_IMAGE_WIDTH = 100;
const DEFAULT_IMAGE_WIDTH = 80;

export function listenForImageResize({
  onCancel,
  onCommit,
  onMove,
}: {
  onCancel: () => void;
  onCommit: () => void;
  onMove: (event: PointerEvent) => void;
}) {
  let active = true;
  const cleanup = () => {
    if (!active) return;
    active = false;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", handleCommit);
    window.removeEventListener("pointercancel", handleCancel);
    window.removeEventListener("blur", handleCancel);
  };
  const handleCommit = () => {
    cleanup();
    onCommit();
  };
  const handleCancel = () => {
    cleanup();
    onCancel();
  };

  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", handleCommit);
  window.addEventListener("pointercancel", handleCancel);
  window.addEventListener("blur", handleCancel);
  return cleanup;
}

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
    sharedAttachmentId: { default: null },
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
          sharedAttachmentId: el.getAttribute("data-shared-attachment-id"),
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
    if (node.attrs.sharedAttachmentId) {
      attrs["data-shared-attachment-id"] = node.attrs.sharedAttachmentId;
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
  const resolveAttachment = useAttachmentResolver();
  const attachmentEditingEnabled = useAttachmentEditingEnabled();
  const attachmentId =
    typeof node.attrs.sharedAttachmentId === "string"
      ? node.attrs.sharedAttachmentId
      : node.attrs.attachmentId;
  const resolvedAttachment =
    typeof attachmentId === "string" ? resolveAttachment?.(attachmentId) : null;
  const [isHovered, setIsHovered] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const [draftWidth, setDraftWidth] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const activeResizeCleanupRef = useRef<(() => void) | null>(null);
  const attachContainer = useCallback((element: HTMLDivElement | null) => {
    if (!element) {
      activeResizeCleanupRef.current?.();
      activeResizeCleanupRef.current = null;
      containerRef.current = null;
      return;
    }

    containerRef.current = element;
    return () => {
      activeResizeCleanupRef.current?.();
      activeResizeCleanupRef.current = null;
      if (containerRef.current === element) {
        containerRef.current = null;
      }
    };
  }, []);
  const updateAttributes = useEditorEventCallback(
    (view, attrs: Record<string, unknown>) => {
      if (!view) return;
      const pos = getSafeNodePos(getPos);
      if (pos === null) return;

      const tr = view.state.tr.setNodeMarkup(pos, undefined, {
        ...node.attrs,
        ...attrs,
      });
      view.dispatch(tr);
    },
  );

  const isSelected = useIsNodeSelected();

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
      if (!attachmentEditingEnabled || !containerEl || !imageEl) return;

      event.preventDefault();
      event.stopPropagation();
      activeResizeCleanupRef.current?.();
      activeResizeCleanupRef.current = null;

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

      const resetDraft = () => {
        setIsResizing(false);
        setDraftWidth(null);
      };
      let releaseListeners = () => {};
      const clearActiveResize = () => {
        if (activeResizeCleanupRef.current === releaseListeners) {
          activeResizeCleanupRef.current = null;
        }
      };
      releaseListeners = listenForImageResize({
        onMove: handlePointerMove,
        onCommit: () => {
          clearActiveResize();
          updateAttributes({
            editorWidth: clampImageWidth((currentWidth / maxWidth) * 100),
          });
          resetDraft();
        },
        onCancel: () => {
          clearActiveResize();
          resetDraft();
        },
      });
      activeResizeCleanupRef.current = releaseListeners;
    },
    [attachmentEditingEnabled, updateAttributes],
  );

  const showControls =
    attachmentEditingEnabled && (isHovered || isSelected || isResizing);
  const editorWidth = clampImageWidth(node.attrs.editorWidth);
  const imageWidth =
    draftWidth !== null ? `${draftWidth}px` : `${editorWidth}%`;
  const imageStyles = stylex.props(
    styles.image,
    isSelected
      ? styles.selectedImage
      : isHovered
        ? styles.hoveredImage
        : undefined,
  );

  return (
    <div ref={ref} {...htmlAttrs} {...stylex.props(styles.root)}>
      <div
        ref={attachContainer}
        {...stylex.props(styles.container, styles.imageWidth(imageWidth))}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={() => setIsHovered(false)}
      >
        <img
          ref={imageRef}
          src={resolvedAttachment?.src ?? node.attrs.src}
          alt={node.attrs.alt || ""}
          title={parseImageMetadata(node.attrs.title).title ?? undefined}
          {...imageStyles}
          className={cn(["prosemirror-image", imageStyles.className])}
          draggable={false}
        />
        {showControls && (
          <>
            <div
              aria-hidden="true"
              {...stylex.props(styles.rightHoverTarget)}
            />
            <div aria-hidden="true" {...stylex.props(styles.leftHoverTarget)} />
            <button
              type="button"
              aria-label="Resize image from left"
              onPointerDown={(event) => handleResizeStart("left", event)}
              {...stylex.props(styles.resizeHandle, styles.leftHandle)}
            >
              <span {...stylex.props(styles.handleGrip)} />
            </button>
            <button
              type="button"
              aria-label="Resize image from right"
              onPointerDown={(event) => handleResizeStart("right", event)}
              {...stylex.props(styles.resizeHandle, styles.rightHandle)}
            >
              <span {...stylex.props(styles.handleGrip)} />
            </button>
          </>
        )}
      </div>
    </div>
  );
});

const styles = stylex.create({
  root: {
    "::selection": {
      backgroundColor: "transparent",
    },
    overflow: "visible",
    position: "relative",
    userSelect: "none",
  },
  container: {
    display: "inline-block",
    maxWidth: "100%",
    overflow: "visible",
    position: "relative",
    width: "fit-content",
  },
  imageWidth: (width: string) => ({
    width,
  }),
  image: {
    backgroundColor: colors.card,
    borderRadius: radii.md,
    maxWidth: "100%",
    transitionDuration: "150ms",
    transitionProperty: "box-shadow, border-color",
    userSelect: "none",
    width: "100%",
  },
  selectedImage: {
    boxShadow: `0 0 0 2px ${colors.card}, 0 0 0 3px color-mix(in oklab, ${colors.foreground} 55%, transparent)`,
  },
  hoveredImage: {
    boxShadow: `0 0 0 2px ${colors.card}, 0 0 0 3px ${colors.border}`,
  },
  rightHoverTarget: {
    height: "100%",
    position: "absolute",
    right: 0,
    top: 0,
    width: "1.5rem",
  },
  leftHoverTarget: {
    height: "100%",
    left: 0,
    position: "absolute",
    top: 0,
    width: "1.5rem",
  },
  resizeHandle: {
    alignItems: "center",
    backdropFilter: "blur(4px)",
    backgroundColor: `color-mix(in oklab, ${colors.card} 95%, transparent)`,
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    cursor: "ew-resize",
    display: "flex",
    height: "3.5rem",
    justifyContent: "center",
    position: "absolute",
    top: "50%",
    transform: "translateY(-50%)",
    width: "1rem",
    boxShadow: shadows.sm,
  },
  leftHandle: {
    left: "0.25rem",
  },
  rightHandle: {
    right: "0.25rem",
  },
  handleGrip: {
    backgroundColor: colors.mutedForeground,
    borderRadius: radii.full,
    height: "2rem",
    width: "0.25rem",
  },
});
