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
import {
  BoldIcon,
  CodeIcon,
  HighlighterIcon,
  ItalicIcon,
  StrikethroughIcon,
} from "lucide-react";
import { toggleMark } from "prosemirror-commands";
import type { MarkType } from "prosemirror-model";
import type { EditorState } from "prosemirror-state";
import { useRef } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { schema } from "../note/schema";

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
  { id: "bold", icon: BoldIcon, markType: schema.marks.bold },
  { id: "italic", icon: ItalicIcon, markType: schema.marks.italic },
  { id: "strike", icon: StrikethroughIcon, markType: schema.marks.strike },
  { id: "code", icon: CodeIcon, markType: schema.marks.code },
  { id: "highlight", icon: HighlighterIcon, markType: schema.marks.highlight },
];

export function FormatToolbar() {
  const toolbarRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  const editorState = useEditorState();
  const shouldShowToolbar = editorState
    ? !editorState.selection.empty && !selectionTouchesTitleHeading(editorState)
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
    const start = view.coordsAtPos(from);
    const end = view.coordsAtPos(to);

    const referenceEl: VirtualElement = {
      getBoundingClientRect: () =>
        new DOMRect(
          Math.min(start.left, end.left),
          start.top,
          Math.abs(end.right - start.left),
          end.bottom - start.top,
        ),
    };

    const update = () => {
      void computePosition(referenceEl, toolbar, {
        placement: "top",
        strategy: "fixed",
        middleware: [offset(8), flip(), shift({ padding: 8 })],
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
      className={cn([
        "border-border bg-card/95 fixed flex items-center gap-0.5 rounded-lg border p-1",
        "shadow-[0_2px_8px_rgba(0,0,0,0.08),0_18px_42px_-16px_rgba(0,0,0,0.34)] backdrop-blur-sm",
      ])}
      style={{ top: 0, left: 0, zIndex: 40 }}
      onMouseDown={(e) => e.preventDefault()}
    >
      {TOOLBAR_BUTTONS.map((button) => {
        const active = isMarkActive(editorState, button.markType);
        return (
          <button
            key={button.id}
            className={cn([
              "flex size-8 items-center justify-center rounded-md",
              "cursor-pointer border-none transition-colors",
              active
                ? "bg-accent text-foreground"
                : "text-muted-foreground hover:bg-accent bg-transparent",
            ])}
            onClick={() => toggle(button.markType)}
          >
            <button.icon className="size-4" />
          </button>
        );
      })}
    </div>,
    document.body,
  );
}
