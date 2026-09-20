import {
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

const STORAGE_KEY = "anarlog:devtools-scan-layout";
export type PanelRect = { x: number; y: number; width: number; height: number };
type Edge = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";
const viewport = () => ({
  width: window.innerWidth,
  height: window.innerHeight,
});

export function constrainPanel(rect: PanelRect, size = viewport()): PanelRect {
  const width = Math.max(
    1,
    Math.min(Math.max(560, rect.width), size.width - 16),
  );
  const height = Math.max(
    1,
    Math.min(Math.max(260, rect.height), size.height - 62),
  );
  return {
    width,
    height,
    x: Math.max(8, Math.min(rect.x, size.width - width - 8)),
    y: Math.max(32, Math.min(rect.y, size.height - height - 30)),
  };
}

export function resizePanel(
  rect: PanelRect,
  edge: Edge,
  dx: number,
  dy: number,
): PanelRect {
  const size = viewport();
  let left = rect.x;
  let top = rect.y;
  let right = rect.x + rect.width;
  let bottom = rect.y + rect.height;
  const minWidth = Math.min(560, size.width - 16);
  const minHeight = Math.min(260, size.height - 62);
  if (edge.includes("w"))
    left = Math.max(8, Math.min(left + dx, right - minWidth));
  if (edge.includes("e"))
    right = Math.min(size.width - 8, Math.max(right + dx, left + minWidth));
  if (edge.includes("n"))
    top = Math.max(32, Math.min(top + dy, bottom - minHeight));
  if (edge.includes("s"))
    bottom = Math.min(size.height - 30, Math.max(bottom + dy, top + minHeight));
  return constrainPanel(
    { x: left, y: top, width: right - left, height: bottom - top },
    size,
  );
}

function defaultRect(): PanelRect {
  return constrainPanel({
    x: window.innerWidth - 908,
    y: window.innerHeight - 490,
    width: 900,
    height: 460,
  });
}
function readRect(): PanelRect {
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    if (
      value &&
      [value.x, value.y, value.width, value.height].every(
        (n) => typeof n === "number" && Number.isFinite(n),
      )
    ) {
      return constrainPanel(value);
    }
  } catch {
    /* A missing preference must not prevent opening diagnostics. */
  }
  return defaultRect();
}
function saveRect(rect: PanelRect): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(rect));
  } catch {
    /* Storage is optional. */
  }
}

/** Write geometry directly so pointer movement does not rerender the profiler. */
export function useScanPanelLayout() {
  const panelRef = useRef<HTMLElement>(null);
  const [initial] = useState(readRect);
  const rect = useRef(initial);
  const gesture = useRef<{
    pointerId: number;
    x: number;
    y: number;
    rect: PanelRect;
    edge?: Edge;
  } | null>(null);
  const apply = (next: PanelRect) => {
    rect.current = next;
    const node = panelRef.current;
    if (!node) return;
    Object.assign(node.style, {
      left: `${next.x}px`,
      top: `${next.y}px`,
      width: `${next.width}px`,
      height: `${next.height}px`,
    });
  };
  useEffect(() => {
    const onResize = () => {
      gesture.current = null;
      apply(constrainPanel(rect.current));
      saveRect(rect.current);
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  const begin = (event: ReactPointerEvent<HTMLElement>, edge?: Edge) => {
    if (event.button !== 0 || gesture.current) return;
    event.preventDefault();
    event.currentTarget.setPointerCapture(event.pointerId);
    gesture.current = {
      pointerId: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      rect: rect.current,
      edge,
    };
  };
  const finish = (event: ReactPointerEvent<HTMLElement>) => {
    if (gesture.current?.pointerId !== event.pointerId) return;
    gesture.current = null;
    saveRect(rect.current);
  };
  return {
    panelRef,
    style: {
      left: initial.x,
      top: initial.y,
      width: initial.width,
      height: initial.height,
    },
    drag: (event: ReactPointerEvent<HTMLElement>) => {
      if (
        (event.target as Element).closest("button, input, select, textarea, a")
      )
        return;
      begin(event);
    },
    resize: begin,
    handlers: {
      onPointerMove(event: ReactPointerEvent<HTMLElement>) {
        const current = gesture.current;
        if (!current || current.pointerId !== event.pointerId) return;
        const dx = event.clientX - current.x;
        const dy = event.clientY - current.y;
        apply(
          current.edge
            ? resizePanel(current.rect, current.edge, dx, dy)
            : constrainPanel({
                ...current.rect,
                x: current.rect.x + dx,
                y: current.rect.y + dy,
              }),
        );
      },
      onPointerUp: finish,
      onPointerCancel: finish,
      onLostPointerCapture: finish,
    },
  };
}

export const RESIZE_HANDLES = [
  ["n", "inset-x-2 top-0 h-1 cursor-ns-resize"],
  ["s", "inset-x-2 bottom-0 h-1 cursor-ns-resize"],
  ["e", "inset-y-2 right-0 w-1 cursor-ew-resize"],
  ["w", "inset-y-2 left-0 w-1 cursor-ew-resize"],
  ["nw", "top-0 left-0 size-6 cursor-nwse-resize"],
  ["ne", "top-0 right-0 size-6 cursor-nesw-resize"],
  ["sw", "bottom-0 left-0 size-6 cursor-nesw-resize"],
  ["se", "bottom-0 right-0 size-6 cursor-nwse-resize"],
] as const;
