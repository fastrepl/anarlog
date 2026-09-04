import {
  autoUpdate,
  computePosition,
  flip,
  offset,
  shift,
  type VirtualElement,
} from "@floating-ui/dom";
import {
  useEditorEffect,
  useEditorEventCallback,
  useEditorState,
} from "@handlewithcare/react-prosemirror";
import { toggleMark } from "prosemirror-commands";
import type { MarkType } from "prosemirror-model";
import type { EditorState } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { useRef } from "react";
import { createPortal } from "react-dom";

import {
  ChatCenteredDots,
  Code,
  Highlighter,
  TextB,
  TextItalic,
  TextStrikethrough,
  TextUnderline,
} from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

import { schema } from "../note/schema";

const OVERFLOW_CLIP = /(auto|scroll|overlay|hidden|clip)/;

export function getClipBoundary(element: Element): Element {
  let current = element.parentElement;
  while (current && current !== document.documentElement) {
    const { overflow, overflowX, overflowY } = getComputedStyle(current);
    if (
      OVERFLOW_CLIP.test(overflowY) ||
      OVERFLOW_CLIP.test(overflowX) ||
      OVERFLOW_CLIP.test(overflow)
    ) {
      return current;
    }
    current = current.parentElement;
  }
  return element;
}

export function createSelectionVirtualElement(
  view: EditorView,
  from: number,
  to: number,
): VirtualElement {
  const start = view.coordsAtPos(from);
  const end = view.coordsAtPos(to);
  return {
    contextElement: view.dom,
    getBoundingClientRect: () =>
      new DOMRect(
        Math.min(start.left, end.left),
        start.top,
        Math.abs(end.right - start.left),
        end.bottom - start.top,
      ),
  };
}

export function selectionTouchesTitleHeading(state: EditorState): boolean {
  const firstNode = state.doc.firstChild;
  if (
    !firstNode ||
    firstNode.type !== state.schema.nodes.heading ||
    firstNode.attrs.level !== 1 ||
    state.selection.empty
  ) {
    return false;
  }

  const titleStart = 1;
  const titleEnd = firstNode.nodeSize - 1;
  const { from, to } = state.selection;

  return from < titleEnd && to > titleStart;
}

function isMarkActive(state: EditorState, type: MarkType): boolean {
  const { from, $from, to, empty } = state.selection;
  if (empty) {
    return !!type.isInSet(state.storedMarks || $from.marks());
  }
  return state.doc.rangeHasMark(from, to, type);
}

const TOOLBAR_BUTTONS: {
  id: string;
  icon: React.ComponentType<{ className?: string }>;
  markType: MarkType;
}[] = [
  { id: "bold", icon: TextB, markType: schema.marks.bold },
  { id: "italic", icon: TextItalic, markType: schema.marks.italic },
  { id: "underline", icon: TextUnderline, markType: schema.marks.underline },
  { id: "strike", icon: TextStrikethrough, markType: schema.marks.strike },
  { id: "code", icon: Code, markType: schema.marks.code },
  { id: "highlight", icon: Highlighter, markType: schema.marks.highlight },
];

export function FormatToolbar({
  onComment,
  showFormatting = true,
}: {
  onComment?: () => void;
  showFormatting?: boolean;
}) {
  const toolbarRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  const editorState = useEditorState();
  const canFormatSelection = editorState
    ? showFormatting && !selectionTouchesTitleHeading(editorState)
    : false;
  const shouldShowToolbar = editorState
    ? !editorState.selection.empty &&
      (canFormatSelection || onComment !== undefined)
    : false;

  const toggle = useEditorEventCallback((view, markType: MarkType) => {
    if (!view) return;
    toggleMark(markType)(view.state, (tr) => view.dispatch(tr));
    view.focus();
  });

  useEditorEffect((view) => {
    if (!view || !shouldShowToolbar) {
      cleanupRef.current?.();
      cleanupRef.current = null;
      return;
    }

    const toolbar = toolbarRef.current;
    if (!toolbar) return;

    const { from, to } = view.state.selection;
    const referenceEl = createSelectionVirtualElement(view, from, to);
    // Portaled toolbars clip to the viewport by default, which includes window
    // chrome. Stay inside the editor scrollport so the menu flips below the
    // first line instead of covering traffic lights.
    const boundary = getClipBoundary(view.dom);

    const update = () => {
      void computePosition(referenceEl, toolbar, {
        placement: "top",
        strategy: "fixed",
        middleware: [
          offset(8),
          flip({
            boundary,
            fallbackPlacements: ["bottom"],
            padding: 8,
          }),
          shift({ boundary, padding: 8 }),
        ],
      }).then(({ x, y }) => {
        Object.assign(toolbar.style, {
          left: `${x}px`,
          top: `${y}px`,
        });
      });
    };

    cleanupRef.current?.();
    cleanupRef.current = autoUpdate(referenceEl, toolbar, update);
    update();
  });

  if (!shouldShowToolbar || !editorState) return null;

  return createPortal(
    <div
      ref={toolbarRef}
      role="toolbar"
      aria-label="Format selection"
      className={cn([
        "bg-popover ring-border fixed z-50 flex items-center gap-0.5 rounded-xl p-1 ring-1",
        "shadow-lg",
      ])}
      style={{ top: 0, left: 0 }}
      onMouseDown={(e) => e.preventDefault()}
    >
      {canFormatSelection &&
        TOOLBAR_BUTTONS.map((button) => {
          const active = isMarkActive(editorState, button.markType);
          return (
            <button
              key={button.id}
              aria-pressed={active}
              className={cn([
                "flex size-7 items-center justify-center rounded-md",
                "cursor-pointer border-none transition-colors",
                active
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground bg-transparent",
              ])}
              onClick={() => toggle(button.markType)}
            >
              <button.icon className="size-4" />
            </button>
          );
        })}
      {canFormatSelection && onComment && (
        <span className="bg-border mx-0.5 h-4 w-px" aria-hidden="true" />
      )}
      {onComment && (
        <button
          type="button"
          aria-label="Comment"
          className={cn([
            "text-muted-foreground flex size-7 items-center justify-center rounded-md",
            "hover:bg-accent hover:text-accent-foreground cursor-pointer border-none bg-transparent transition-colors",
          ])}
          onClick={onComment}
        >
          <ChatCenteredDots className="size-4" />
        </button>
      )}
    </div>,
    document.body,
  );
}
