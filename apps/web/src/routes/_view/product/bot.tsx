import { createFileRoute } from "@tanstack/react-router";
import { useCallback, useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

export const Route = createFileRoute("/_view/product/bot")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Bot - Char" },
      {
        name: "description",
        content: "Char Bot for meeting platforms. Coming soon.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const DRAGGABLE_ICONS = [
  { left: "15%", top: "20%", rotate: "-8deg", size: 80 },
  { left: "75%", top: "25%", rotate: "5deg", size: 96 },
  { left: "20%", top: "65%", rotate: "12deg", size: 72 },
  { left: "80%", top: "70%", rotate: "-6deg", size: 88 },
];

function Component() {
  return (
    <div
      className="relative h-[calc(100vh-65px)] overflow-hidden bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="relative mx-auto h-full max-w-6xl border-x border-neutral-100 bg-white">
        {DRAGGABLE_ICONS.map((icon, idx) => (
          <DraggableIcon
            key={idx}
            initialPosition={{
              left: icon.left,
              top: icon.top,
              rotate: icon.rotate,
            }}
            size={icon.size}
          />
        ))}

        <div className="pointer-events-none relative z-10 flex h-full items-center justify-center bg-[linear-gradient(to_bottom,rgba(245,245,244,0.2),white_50%,rgba(245,245,244,0.3))] px-6">
          <div className="pointer-events-auto mx-auto max-w-4xl text-center">
            <h1 className="mb-6 font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Char Bot
            </h1>
            <p className="text-lg text-neutral-600 sm:text-xl">
              An optional bot solution for teams that need centralized meeting
              recording and management.
            </p>
            <div className="mt-8">
              <button
                disabled
                className={cn([
                  "inline-block cursor-not-allowed px-8 py-3 text-base font-medium",
                  "rounded-full bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900 shadow-xs",
                ])}
              >
                Coming Soon
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function DraggableIcon({
  initialPosition,
  size,
}: {
  initialPosition: { left: string; top: string; rotate: string };
  size: number;
}) {
  const [dragStart, setDragStart] = useState<{ x: number; y: number } | null>(
    null,
  );
  const [position, setPosition] = useState({ x: 0, y: 0 });
  const [scale, setScale] = useState(1);
  const iconRef = useRef<HTMLDivElement>(null);

  const calculatePosition = useCallback(() => {
    if (typeof window === "undefined") {
      return { x: 0, y: 0 };
    }

    const container = iconRef.current?.parentElement;
    if (!container) {
      return { x: 0, y: 0 };
    }

    const containerWidth = container.clientWidth;
    const containerHeight = container.clientHeight;

    const x =
      (containerWidth * parseInt(initialPosition.left)) / 100 - size / 2;
    const y =
      (containerHeight * parseInt(initialPosition.top)) / 100 - size / 2;

    return { x, y };
  }, [initialPosition, size]);

  useEffect(() => {
    setPosition(calculatePosition());
  }, [calculatePosition]);

  useEffect(() => {
    const handleResize = () => {
      if (!dragStart) {
        setPosition(calculatePosition());
      }
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, [calculatePosition, dragStart]);

  const handlePointerDown = (e: React.PointerEvent) => {
    if (!iconRef.current) {
      return;
    }

    iconRef.current.setPointerCapture(e.pointerId);
    setScale(1.05);
    setDragStart({
      x: e.clientX - position.x,
      y: e.clientY - position.y,
    });
  };

  const handlePointerMove = (e: React.PointerEvent) => {
    if (!dragStart || !iconRef.current?.hasPointerCapture(e.pointerId)) {
      return;
    }

    setPosition({
      x: e.clientX - dragStart.x,
      y: e.clientY - dragStart.y,
    });
  };

  const handlePointerUp = (e: React.PointerEvent) => {
    if (!iconRef.current) {
      return;
    }

    iconRef.current.releasePointerCapture(e.pointerId);
    setScale(1);
    setDragStart(null);
  };

  return (
    <div
      ref={iconRef}
      className="absolute cursor-grab active:cursor-grabbing"
      style={{
        width: `${size}px`,
        height: `${size}px`,
        left: `${position.x}px`,
        top: `${position.y}px`,
        transform: `scale(${scale}) rotate(${initialPosition.rotate})`,
        transition: dragStart ? "none" : "transform 0.2s ease-out",
        zIndex: 20,
        pointerEvents: "auto",
        userSelect: "none",
        touchAction: "none",
      }}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
    >
      <div className="flex h-full w-full items-center justify-center rounded-3xl border border-neutral-100 bg-transparent shadow-2xl">
        <img
          src="/api/images/hyprnote/icon.png"
          alt="Char"
          className="h-[85%] w-[85%] rounded-[20px] border border-neutral-100"
          draggable={false}
        />
      </div>
    </div>
  );
}
