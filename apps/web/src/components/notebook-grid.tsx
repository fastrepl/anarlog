import { useCallback, useEffect, useRef, useState } from "react";

const TARGET_CELL_SIZE = 40;
const ROTATIONS = [0, 90, 180, 270] as const;

function CellPattern() {
  return (
    <svg width="100%" height="100%" viewBox="0 0 56 56" className="size-full">
      {[39, 32, 25, 17].map((r, i) => (
        <circle
          key={i}
          cx={0}
          cy={56}
          r={r}
          fill="none"
          stroke="var(--color-border-bright)"
          opacity={0.5}
          strokeWidth={2}
        />
      ))}
    </svg>
  );
}

export function NotebookGrid() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [cellSize, setCellSize] = useState(TARGET_CELL_SIZE);
  const [dims, setDims] = useState({ cols: 0, rows: 0 });
  const [rotations, setRotations] = useState<number[]>([]);
  const currentCellIdx = useRef(-1);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const obs = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      const cols = Math.max(1, Math.round(width / TARGET_CELL_SIZE));
      const size = width / cols;
      const rows = size > 0 ? Math.ceil(height / size) : 0;
      const count = cols * rows;
      setCellSize(size);
      setDims({ cols, rows });
      setRotations(
        Array.from(
          { length: count },
          () => ROTATIONS[Math.floor(Math.random() * 4)],
        ),
      );
      currentCellIdx.current = -1;
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      const el = containerRef.current;
      if (!el || dims.cols === 0 || cellSize === 0) return;
      const rect = el.getBoundingClientRect();
      const cellX = Math.floor((e.clientX - rect.left) / cellSize);
      const cellY = Math.floor((e.clientY - rect.top) / cellSize);
      if (cellX < 0 || cellY < 0 || cellX >= dims.cols || cellY >= dims.rows)
        return;
      const idx = cellY * dims.cols + cellX;
      if (idx === currentCellIdx.current) return;
      currentCellIdx.current = idx;
      setRotations((prev) => {
        const next = [...prev];
        next[idx] = (next[idx] ?? 0) + 90;
        return next;
      });
    },
    [dims, cellSize],
  );

  const handleMouseLeave = useCallback(() => {
    currentCellIdx.current = -1;
  }, []);

  return (
    <div
      ref={containerRef}
      className="border-color-brand h-full w-full overflow-hidden border"
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
      style={{
        display: "grid",
        gridTemplateColumns: `repeat(${dims.cols}, 1fr)`,
      }}
    >
      {rotations.map((rotation, i) => (
        <div
          key={i}
          className="relative overflow-hidden"
          style={{
            boxShadow: "0px 0px 0px 0.5px var(--color-border)",
          }}
        >
          <div
            className="absolute inset-0 transition-transform duration-300 ease-in-out"
            style={{ transform: `rotate(${rotation}deg)` }}
          >
            <CellPattern />
          </div>
        </div>
      ))}
    </div>
  );
}
