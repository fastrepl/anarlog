type Outline = {
  node: Element;
  names: Set<string>;
  count: number;
  alpha: number;
};

const OUTLINE_RGB = "115, 97, 230";
const FADE_PER_FRAME = 1 / 40;
const LABEL_HEIGHT = 12;
const LABEL_FONT = "10px ui-monospace, SFMono-Regular, Menlo, monospace";

const outlines = new Map<Element, Outline>();
let canvas: HTMLCanvasElement | null = null;
let frame: number | null = null;

/** Highlights the DOM nodes of components that just rendered; repeated flashes bump the count. */
export function flashRenderOutlines(
  entries: Array<{ node: Element; name: string }>,
): void {
  for (const { node, name } of entries) {
    const existing = outlines.get(node);
    if (existing) {
      existing.count += 1;
      existing.alpha = 1;
      existing.names.add(name);
    } else {
      outlines.set(node, { node, names: new Set([name]), count: 1, alpha: 1 });
    }
  }
  if (frame === null && outlines.size) {
    frame = requestAnimationFrame(draw);
  }
}

export function hideRenderOutlines(): void {
  outlines.clear();
  if (frame !== null) {
    cancelAnimationFrame(frame);
    frame = null;
  }
  if (canvas) {
    window.removeEventListener("resize", resizeCanvas);
    canvas.remove();
    canvas = null;
  }
}

function ensureCanvas(): HTMLCanvasElement {
  if (canvas) return canvas;

  canvas = document.createElement("canvas");
  canvas.setAttribute("aria-hidden", "true");
  canvas.style.cssText =
    "position:fixed;inset:0;pointer-events:none;z-index:2147483647;";
  document.body.appendChild(canvas);
  window.addEventListener("resize", resizeCanvas);
  resizeCanvas();
  return canvas;
}

function resizeCanvas() {
  if (!canvas) return;
  const scale = window.devicePixelRatio || 1;
  canvas.width = Math.round(window.innerWidth * scale);
  canvas.height = Math.round(window.innerHeight * scale);
  canvas.style.width = `${window.innerWidth}px`;
  canvas.style.height = `${window.innerHeight}px`;
  canvas.getContext("2d")?.setTransform(scale, 0, 0, scale, 0, 0);
}

function draw() {
  frame = null;
  const context = ensureCanvas().getContext("2d");
  if (!context) return;

  context.clearRect(0, 0, window.innerWidth, window.innerHeight);
  context.font = LABEL_FONT;
  context.textBaseline = "middle";
  context.lineWidth = 1;

  for (const outline of outlines.values()) {
    if (!outline.node.isConnected) {
      outlines.delete(outline.node);
      continue;
    }

    const rect = outline.node.getBoundingClientRect();
    if (rect.width > 0 || rect.height > 0) {
      const { alpha } = outline;
      context.strokeStyle = `rgba(${OUTLINE_RGB}, ${alpha})`;
      context.fillStyle = `rgba(${OUTLINE_RGB}, ${alpha * 0.08})`;
      context.fillRect(rect.x, rect.y, rect.width, rect.height);
      context.strokeRect(
        rect.x + 0.5,
        rect.y + 0.5,
        Math.max(rect.width - 1, 0),
        Math.max(rect.height - 1, 0),
      );

      const label =
        [...outline.names].join(", ") +
        (outline.count > 1 ? ` ×${outline.count}` : "");
      const width = context.measureText(label).width + 6;
      const x = Math.max(rect.x, 0);
      const y = rect.y >= LABEL_HEIGHT ? rect.y - LABEL_HEIGHT : rect.y;
      context.fillStyle = `rgba(${OUTLINE_RGB}, ${alpha})`;
      context.fillRect(x, y, width, LABEL_HEIGHT);
      context.fillStyle = `rgba(255, 255, 255, ${alpha})`;
      context.fillText(label, x + 3, y + LABEL_HEIGHT / 2);
    }

    outline.alpha -= FADE_PER_FRAME;
    if (outline.alpha <= 0) {
      outlines.delete(outline.node);
    }
  }

  if (outlines.size) {
    frame = requestAnimationFrame(draw);
  } else {
    context.clearRect(0, 0, window.innerWidth, window.innerHeight);
  }
}
