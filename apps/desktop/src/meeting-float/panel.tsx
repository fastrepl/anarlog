import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { MouseEvent } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import {
  useFloatingMeetingTitle,
  useFloatingWaveformState,
  useMirrorFloatingCaptureState,
} from "./hooks";
import {
  FLOATING_MEETING_PANEL_MARGIN,
  hideFloatingMeetingPanel,
} from "./host";

import { listenerStore } from "~/store/zustand/listener/instance";

export function FloatingMeetingPanel({
  sessionId,
  initialShowTranscriptTab,
}: {
  sessionId: string;
  initialShowTranscriptTab: boolean;
}) {
  useMirrorFloatingCaptureState(sessionId, initialShowTranscriptTab);

  const title = useFloatingMeetingTitle(sessionId);

  return (
    <div
      className="h-screen w-screen overflow-hidden bg-transparent"
      style={{ padding: FLOATING_MEETING_PANEL_MARGIN }}
    >
      <GlassBar title={title} />
    </div>
  );
}

function GlassBar({ title }: { title: string }) {
  return (
    <div
      data-tauri-drag-region
      onMouseDown={startWindowDrag}
      className={cn([
        "relative flex h-full w-full items-center gap-2 overflow-hidden rounded-[20px] px-2.5",
        "border border-white/18 bg-[rgba(72,74,68,0.72)] backdrop-blur-2xl backdrop-saturate-150",
        "before:pointer-events-none before:absolute before:inset-0 before:rounded-[inherit]",
        "before:bg-linear-to-br before:from-white/18 before:via-white/6 before:to-transparent",
        "after:pointer-events-none after:absolute after:inset-px after:rounded-[19px] after:border after:border-white/8",
      ])}
    >
      <span
        aria-hidden="true"
        className="relative z-10 -mt-0.5 w-6 shrink-0 text-center text-[31px] leading-none font-normal"
        style={{
          color: "#fff",
          fontFamily: '"Cabin Sketch", var(--font-sans)',
        }}
      >
        a
      </span>
      <LiveWaveform className="relative z-10" />
      <span className="sr-only">{title}</span>
    </div>
  );
}

function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) {
    return;
  }

  void getCurrentWebviewWindow()
    .startDragging()
    .catch((error) => {
      console.error("Failed to drag floating meeting panel:", error);
    });
}

function stopWindowDrag(event: MouseEvent<HTMLElement>) {
  event.stopPropagation();
}

function LiveWaveform({ className }: { className?: string }) {
  const { amplitude, degraded } = useFloatingWaveformState();
  const color = degraded ? "#f59e0b" : "#ff333b";

  return (
    <button
      type="button"
      aria-label="Stop listening"
      title="Stop listening"
      onClick={stopListening}
      onMouseDown={stopWindowDrag}
      data-tauri-drag-region="false"
      className={cn([
        "group relative h-5 w-16 shrink-0 overflow-hidden rounded-[6px] p-0",
        "text-white/90 transition-colors hover:bg-white/12 hover:text-white",
        className,
      ])}
    >
      <span className="absolute inset-0 flex items-center justify-center group-hover:hidden">
        <DancingSticks
          amplitude={Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1)}
          color={color}
          height={18}
          width={60}
        />
      </span>
      <span className="absolute inset-0 hidden items-center justify-center gap-1.5 text-xs leading-none font-semibold group-hover:flex">
        <span
          className="size-2 rounded-none"
          style={{ backgroundColor: color }}
        />
        <span>Stop</span>
      </span>
    </button>
  );
}

function stopListening() {
  void hideFloatingMeetingPanel();
  listenerStore.getState().stop();
}
