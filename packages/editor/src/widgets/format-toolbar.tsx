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
  const hasSelection = editorState ? !editorState.selection.empty : false;

  const toggle = useEditorEventCallback((view, markType: MarkType) => {
    if (!view) return;
    toggleMark(markType)(view.state, (tr) => view.dispatch(tr));
    view.focus();
  });

  useEditorEffect((view) => {
    if (!view || !hasSelection) {
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

  if (!hasSelection || !editorState) return null;

  return createPortal(
    <div
      ref={toolbarRef}
      className={cn([
        "absolute z-[9999] flex items-center gap-0.5 rounded-lg bg-white p-1",
        "shadow-[0_0_0_1px_rgba(0,0,0,0.05),0_6px_12px_-3px_rgba(0,0,0,0.08)]",
      ])}
      style={{ top: 0, left: 0 }}
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
                ? "bg-neutral-200 text-neutral-900"
                : "bg-transparent text-neutral-600 hover:bg-neutral-100",
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
