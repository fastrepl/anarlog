import { createFileRoute } from "@tanstack/react-router";
import {
  AlertTriangle,
  ChevronDown,
  Eye,
  Mic,
  MicOff,
  Pin,
  PinOff,
  Square,
} from "lucide-react";
import { useRef, useState } from "react";

import { commands as windowsCommands } from "@hypr/plugin-windows";
import { Button } from "@hypr/ui/components/ui/button";
import { Slider } from "@hypr/ui/components/ui/slider";
import { cn } from "@hypr/utils";

import { useWidgetState } from "~/shared/hooks/useWidgetState";
import { useListener } from "~/stt/contexts";

export const Route = createFileRoute("/app/control")({
  component: Component,
});

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}

function Component() {
  const { isExpanded, expand, collapse } = useWidgetState();

  const { status, seconds, muted, amplitude, degraded } = useListener(
    (state) => ({
      status: state.live.status,
      seconds: state.live.seconds,
      muted: state.live.muted,
      amplitude: state.live.amplitude,
      degraded: state.live.degraded,
    }),
  );

  const { stop, setMuted } = useListener((state) => ({
    stop: state.stop,
    setMuted: state.setMuted,
  }));

  const isFinalizing = status === "finalizing";

  if (!isExpanded) {
    return (
      <CollapsedPill
        onExpand={expand}
        seconds={seconds}
        isFinalizing={isFinalizing}
      />
    );
  }

  return (
    <ExpandedPanel
      onCollapse={collapse}
      isFinalizing={isFinalizing}
      seconds={seconds}
      muted={muted}
      amplitude={amplitude}
      degraded={degraded !== null}
      stop={stop}
      setMuted={setMuted}
    />
  );
}

function CollapsedPill({
  onExpand,
  seconds,
  isFinalizing,
}: {
  onExpand: () => void;
  seconds: number;
  isFinalizing: boolean;
}) {
  const mouseDownPos = useRef<{ x: number; y: number } | null>(null);

  const handleMouseDown = (e: React.MouseEvent) => {
    mouseDownPos.current = { x: e.clientX, y: e.clientY };
  };

  const handleMouseUp = (e: React.MouseEvent) => {
    if (!mouseDownPos.current) return;

    const dx = Math.abs(e.clientX - mouseDownPos.current.x);
    const dy = Math.abs(e.clientY - mouseDownPos.current.y);
    const wasDrag = dx > 5 || dy > 5;

    mouseDownPos.current = null;

    if (!wasDrag) {
      onExpand();
    }
  };

  const handleMouseLeave = () => {
    mouseDownPos.current = null;
  };

  return (
    <div
      data-tauri-drag-region
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseLeave}
      className={cn([
        "flex h-full w-full cursor-pointer items-center justify-center gap-2",
        "rounded-full bg-black/60 backdrop-blur-sm",
        "px-3",
      ])}
    >
      <div
        data-tauri-drag-region
        className={cn([
          "h-2 w-2 shrink-0 animate-pulse rounded-full",
          isFinalizing ? "bg-yellow-500" : "bg-red-500",
        ])}
      />
      <span
        data-tauri-drag-region
        className="font-mono text-xs font-medium text-white"
      >
        {formatTime(seconds)}
      </span>
    </div>
  );
}

function ExpandedPanel({
  onCollapse,
  isFinalizing,
  seconds,
  muted,
  amplitude,
  degraded,
  stop,
  setMuted,
}: {
  onCollapse: () => void;
  isFinalizing: boolean;
  seconds: number;
  muted: boolean;
  amplitude: { mic: number };
  degraded: boolean;
  stop: () => void;
  setMuted: (muted: boolean) => void;
}) {
  return (
    <div
      className={cn([
        "flex h-full w-full flex-col",
        "rounded-2xl bg-black/80 backdrop-blur-xl",
      ])}
    >
      <header
        data-tauri-drag-region
        className={cn([
          "flex shrink-0 items-center justify-between",
          "px-4 pt-3 pb-2",
        ])}
      >
        <div data-tauri-drag-region className="flex items-center gap-2">
          <div
            data-tauri-drag-region
            className={cn([
              "h-2 w-2 shrink-0 animate-pulse rounded-full",
              isFinalizing ? "bg-yellow-500" : "bg-red-500",
            ])}
          />
          <span
            data-tauri-drag-region
            className="font-mono text-sm font-medium text-white"
          >
            {formatTime(seconds)}
          </span>
          {degraded && (
            <AlertTriangle className="h-3.5 w-3.5 text-yellow-400" />
          )}
        </div>
        <div className="flex items-center gap-1">
          <PinToggle />
          <OpacityControl />
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 text-white/40 hover:bg-white/10 hover:text-white"
            onClick={onCollapse}
          >
            <ChevronDown className="h-3.5 w-3.5" />
          </Button>
        </div>
      </header>

      <div className="mx-4 border-t border-white/10" />

      <div
        data-tauri-drag-region
        className="flex flex-1 flex-col items-center justify-center gap-6 px-4"
      >
        <span
          data-tauri-drag-region
          className="font-mono text-5xl font-light tracking-wider text-white"
        >
          {formatTime(seconds)}
        </span>

        <div data-tauri-drag-region className="flex items-end gap-1">
          {[...Array(12)].map((_, i) => (
            <div
              key={i}
              className={cn([
                "w-1 rounded-full transition-all duration-100",
                amplitude.mic > i * 0.08 ? "bg-green-400" : "bg-white/10",
              ])}
              style={{
                height: `${Math.max(4, Math.min(32, amplitude.mic * 120 * (0.5 + Math.sin(i * 0.8) * 0.5)))}px`,
              }}
            />
          ))}
        </div>
      </div>

      <div className="mx-4 border-t border-white/10" />

      <div className="flex shrink-0 items-center justify-center gap-3 px-4 py-3">
        <Button
          variant="ghost"
          size="icon"
          className={cn([
            "h-10 w-10 rounded-full",
            muted
              ? "bg-red-500/20 text-red-400 hover:bg-red-500/30"
              : "bg-white/10 text-white hover:bg-white/20",
          ])}
          onClick={() => setMuted(!muted)}
        >
          {muted ? <MicOff className="h-5 w-5" /> : <Mic className="h-5 w-5" />}
        </Button>

        <Button
          variant="ghost"
          size="icon"
          className="h-12 w-12 rounded-full bg-red-500/20 text-red-400 hover:bg-red-500/30"
          onClick={stop}
          disabled={isFinalizing}
        >
          <Square className="h-5 w-5 fill-current" />
        </Button>
      </div>
    </div>
  );
}

function PinToggle() {
  const [pinned, setPinned] = useState(true);

  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn([
        "h-6 w-6",
        pinned
          ? "text-white/70 hover:bg-white/10 hover:text-white"
          : "text-white/30 hover:bg-white/10 hover:text-white/50",
      ])}
      onClick={() => {
        const next = !pinned;
        setPinned(next);
        windowsCommands.controlSetAlwaysOnTop(next);
      }}
    >
      {pinned ? (
        <Pin className="h-3.5 w-3.5" />
      ) : (
        <PinOff className="h-3.5 w-3.5" />
      )}
    </Button>
  );
}

function OpacityControl() {
  const [showSlider, setShowSlider] = useState(false);
  const [opacity, setOpacity] = useState(100);

  return (
    <div className="relative flex items-center">
      <Button
        variant="ghost"
        size="icon"
        className="h-6 w-6 text-white/40 hover:bg-white/10 hover:text-white"
        onClick={() => setShowSlider(!showSlider)}
      >
        <Eye className="h-3.5 w-3.5" />
      </Button>
      {showSlider && (
        <div className="absolute top-full right-0 mt-1 flex items-center gap-2 rounded-lg bg-black/90 px-3 py-2">
          <Slider
            className="w-24"
            min={20}
            max={100}
            step={5}
            value={[opacity]}
            onValueChange={([v]) => {
              setOpacity(v);
              windowsCommands.controlSetOpacity(v / 100);
            }}
          />
          <span className="min-w-[2ch] text-right font-mono text-xs text-white/50">
            {opacity}
          </span>
        </div>
      )}
    </div>
  );
}
