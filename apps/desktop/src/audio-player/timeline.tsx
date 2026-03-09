import { Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { useAudioPlayer, useAudioTime } from "./provider";

const PLAYBACK_RATES = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

export function Timeline() {
  const {
    registerContainer,
    state,
    pause,
    resume,
    start,
    playbackRate,
    setPlaybackRate,
  } = useAudioPlayer();
  const time = useAudioTime();
  const [showRateMenu, setShowRateMenu] = useState(false);
  const rateMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (
        rateMenuRef.current &&
        !rateMenuRef.current.contains(e.target as Node)
      ) {
        setShowRateMenu(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  const handleClick = () => {
    if (state === "playing") {
      pause();
    } else if (state === "paused") {
      resume();
    } else if (state === "stopped") {
      start();
    }
  };

  return (
    <div className="w-full rounded-xl bg-neutral-50">
      <div className={cn(["flex items-center gap-2 p-2", "w-full max-w-full"])}>
        <button
          onClick={handleClick}
          className={cn([
            "flex items-center justify-center",
            "h-8 w-8 rounded-full",
            "border border-neutral-200 bg-white",
            "transition-all hover:scale-110 hover:bg-neutral-100",
            "shrink-0 shadow-xs",
          ])}
        >
          {state === "playing" ? (
            <Pause className="h-4 w-4 text-neutral-900" fill="currentColor" />
          ) : (
            <Play className="h-4 w-4 text-neutral-900" fill="currentColor" />
          )}
        </button>

        <div className="inline-flex shrink-0 items-center gap-1 font-mono text-xs text-neutral-600 tabular-nums">
          <span>{formatTime(time.current)}</span>/
          <span>{formatTime(time.total)}</span>
        </div>

        <div className="relative shrink-0" ref={rateMenuRef}>
          <button
            onClick={() => setShowRateMenu((prev) => !prev)}
            className={cn([
              "flex items-center justify-center",
              "h-6 rounded-md px-1.5",
              "border border-neutral-200 bg-white",
              "transition-colors hover:bg-neutral-100",
              "font-mono text-xs text-neutral-700",
              "shadow-xs",
            ])}
          >
            {playbackRate}x
          </button>
          {showRateMenu && (
            <div
              className={cn([
                "absolute right-0 bottom-full mb-1",
                "rounded-lg border border-neutral-200 bg-white shadow-md",
                "z-50 py-1",
              ])}
            >
              {PLAYBACK_RATES.map((rate) => (
                <button
                  key={rate}
                  onClick={() => {
                    setPlaybackRate(rate);
                    setShowRateMenu(false);
                  }}
                  className={cn([
                    "block w-full px-3 py-1 text-left font-mono text-xs",
                    "transition-colors hover:bg-neutral-100",
                    rate === playbackRate
                      ? "font-semibold text-neutral-900"
                      : "text-neutral-600",
                  ])}
                >
                  {rate}x
                </button>
              ))}
            </div>
          )}
        </div>

        <div
          ref={registerContainer}
          className="min-w-0 flex-1"
          style={{ minHeight: "30px", width: "100%" }}
        />
      </div>
    </div>
  );
}

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
}
