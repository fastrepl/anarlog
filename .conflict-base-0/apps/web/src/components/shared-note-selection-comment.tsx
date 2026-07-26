import {
  autoUpdate,
  computePosition,
  flip,
  offset,
  shift,
} from "@floating-ui/dom";
import { MessageSquarePlusIcon } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

export type SelectionRect = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

export function SharedNoteSelectionComment({
  onStart,
  rect,
  visible,
}: {
  rect: SelectionRect | null;
  visible: boolean;
  onStart: () => void;
}) {
  const [pill, setPill] = useState<HTMLButtonElement | null>(null);
  const rectRef = useRef(rect);
  rectRef.current = rect;

  const active = visible && rect !== null;

  // External sync: floating-ui keeps the portal pill aligned with the
  // viewport rect of the current selection.
  useEffect(() => {
    if (!pill || !active) return;
    const reference = {
      getBoundingClientRect: () => {
        const current = rectRef.current ?? {
          bottom: 0,
          left: 0,
          right: 0,
          top: 0,
        };
        return {
          ...current,
          x: current.left,
          y: current.top,
          width: current.right - current.left,
          height: current.bottom - current.top,
        };
      },
    };
    const update = () => {
      void computePosition(reference, pill, {
        middleware: [offset(8), flip(), shift({ padding: 8 })],
        placement: "top",
        strategy: "fixed",
      }).then(({ x, y }) => {
        pill.style.left = `${x}px`;
        pill.style.top = `${y}px`;
        pill.style.visibility = "visible";
      });
    };
    return autoUpdate(reference, pill, update);
  }, [pill, active, rect]);

  if (!active) return null;

  return createPortal(
    <button
      ref={setPill}
      type="button"
      style={{ left: 0, position: "fixed", top: 0, visibility: "hidden" }}
      className={cn([
        "z-50 inline-flex items-center gap-1.5",
        "surface border-color-subtle rounded-full border px-3 py-1.5 shadow-md",
        "text-color font-mono text-xs font-medium",
        "hover:bg-surface-subtle transition-colors",
        "focus-visible:ring-2 focus-visible:ring-stone-500 focus-visible:outline-hidden",
      ])}
      onMouseDown={(event) => event.preventDefault()}
      onClick={onStart}
    >
      <MessageSquarePlusIcon className="size-4" aria-hidden="true" />
      Comment
    </button>,
    document.body,
  );
}
