import { useEffect, useRef, useState } from "react";

import type {
  FloatingBarState,
  FloatingTranscriptBubble,
} from "@anlg/plugin-windows";
import {
  ArrowsInSimple,
  ArrowsOutSimple,
  CaretDown,
  CircleNotch,
  Square,
  WarningCircle,
} from "@anlg/ui/components/icons";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { cn } from "@anlg/utils";

import { DictationTranscript } from "./dictation";
import {
  FLOATING_BAR_COMPACT_GAP,
  FLOATING_BAR_COMPACT_HEIGHT,
  FLOATING_BAR_COMPACT_HORIZONTAL_PADDING,
  FLOATING_BAR_COMPACT_ICON_SIZE,
  FLOATING_BAR_COMPACT_SOLO_STOP_WIDTH,
  FLOATING_BAR_COMPACT_STOP_WIDTH,
  FLOATING_BAR_COMPACT_RADIUS,
  FLOATING_BAR_CONTROL_RADIUS,
  FLOATING_BAR_EXPANDED_RADIUS,
  FLOATING_BAR_HOVER_HANDLE_HEIGHT,
  FLOATING_BAR_HOVER_HANDLE_TOP_PADDING,
  FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT,
  FLOATING_BAR_INSET,
  compactControlsWidth,
} from "./layout";

export function FloatingBarOverlay({
  state,
  onStop,
  onToggleExpanded,
}: {
  state: FloatingBarState;
  onStop: () => void;
  onToggleExpanded: (expanded: boolean) => void;
}) {
  const [hovered, setHovered] = useState(false);
  const isExpanded =
    state.liveCaptionToggleVisible && !state.liveCaptionMinimized;
  const showsHoverHandle = hovered && !isExpanded;
  const colors = barColors(state);
  const controlsWidth = compactControlsWidth(state.liveCaptionToggleVisible);
  const expandsUpward = state.layout?.expandsUpward ?? false;

  return (
    <div
      className="relative h-full w-full"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {isExpanded && (
        <div
          data-tauri-drag-region
          className="absolute inset-x-0 top-0"
          style={{
            height:
              FLOATING_BAR_INSET + FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT,
          }}
        />
      )}
      <div
        className="absolute overflow-hidden"
        style={{
          left: FLOATING_BAR_INSET,
          right: FLOATING_BAR_INSET,
          bottom: FLOATING_BAR_INSET,
          top:
            FLOATING_BAR_INSET +
            (showsHoverHandle ? 0 : FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT),
          borderRadius: isExpanded
            ? FLOATING_BAR_EXPANDED_RADIUS
            : FLOATING_BAR_COMPACT_RADIUS,
          background:
            hovered && !isExpanded ? colors.envelopeSurface : colors.surface,
          boxShadow: `inset 0 0 0 0.5px ${colors.outerStroke}`,
        }}
      >
        {showsHoverHandle && <HoverHandle color={colors.handle} />}
        <div
          className="absolute inset-x-0 bottom-0"
          style={{
            top: showsHoverHandle
              ? FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT
              : 0,
          }}
        >
          {isExpanded && (
            <div
              className="absolute inset-x-0"
              style={{
                top: expandsUpward ? 0 : FLOATING_BAR_COMPACT_HEIGHT,
                bottom: expandsUpward ? FLOATING_BAR_COMPACT_HEIGHT : 0,
              }}
            >
              <TranscriptList
                key={state.dictation?.sessionId ?? "meeting"}
                dictation={state.dictation}
                bubbles={state.transcriptBubbles ?? []}
                colorScheme={state.colorScheme}
              />
            </div>
          )}
          <div
            className="absolute flex -translate-x-1/2 items-center justify-center"
            style={{
              left: state.layout
                ? state.layout.controlsCenterX - FLOATING_BAR_INSET
                : `calc(100% - ${FLOATING_BAR_COMPACT_HORIZONTAL_PADDING + controlsWidth / 2}px)`,
              top: expandsUpward ? undefined : 0,
              bottom: expandsUpward ? 0 : undefined,
              width: controlsWidth,
              height: FLOATING_BAR_COMPACT_HEIGHT,
            }}
          >
            <FloatingControls
              state={state}
              isExpanded={isExpanded}
              colors={colors}
              onStop={onStop}
              onToggleExpanded={onToggleExpanded}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function FloatingControls({
  state,
  isExpanded,
  colors,
  onStop,
  onToggleExpanded,
}: {
  state: FloatingBarState;
  isExpanded: boolean;
  colors: BarColors;
  onStop: () => void;
  onToggleExpanded: (expanded: boolean) => void;
}) {
  return (
    <div
      className="flex items-center"
      style={{ gap: FLOATING_BAR_COMPACT_GAP }}
    >
      <StopControl state={state} colors={colors} onStop={onStop} />
      {state.liveCaptionToggleVisible ? (
        <button
          type="button"
          data-tauri-drag-region="false"
          aria-label={
            isExpanded ? "Collapse live transcript" : "Expand live transcript"
          }
          onClick={() => onToggleExpanded(!isExpanded)}
          className="flex items-center justify-center"
          style={{
            width: FLOATING_BAR_COMPACT_ICON_SIZE,
            height: FLOATING_BAR_COMPACT_ICON_SIZE,
            borderRadius: FLOATING_BAR_CONTROL_RADIUS,
            color: colors.content,
          }}
        >
          {isExpanded ? (
            <ArrowsInSimple size={14} weight="bold" />
          ) : (
            <ArrowsOutSimple size={14} weight="bold" />
          )}
        </button>
      ) : null}
    </div>
  );
}

function StopControl({
  state,
  colors,
  onStop,
}: {
  state: FloatingBarState;
  colors: BarColors;
  onStop: () => void;
}) {
  const [hovered, setHovered] = useState(false);
  const width = state.liveCaptionToggleVisible
    ? FLOATING_BAR_COMPACT_STOP_WIDTH
    : FLOATING_BAR_COMPACT_SOLO_STOP_WIDTH;

  return (
    <button
      type="button"
      data-tauri-drag-region="false"
      aria-label={
        state.dictation
          ? "Finish dictation"
          : state.status === "reconnecting"
            ? "Reconnecting live transcription; stop listening"
            : state.status === "error"
              ? "Transcription unavailable; stop listening"
              : "Stop listening"
      }
      disabled={state.dictation?.phase === "transcribing"}
      onClick={onStop}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="flex items-center justify-center"
      style={{
        width,
        height: FLOATING_BAR_COMPACT_ICON_SIZE,
        borderRadius: FLOATING_BAR_CONTROL_RADIUS,
        background: hovered ? "rgba(255, 51, 77, 0.18)" : colors.controlFill,
        color: colors.accent,
      }}
    >
      {state.dictation?.phase === "transcribing" ? (
        <span role="status" className="text-xs">
          Finishing…
        </span>
      ) : hovered ? (
        <span className="flex items-center gap-1.5 text-xs font-semibold">
          <Square size={9} />
          {state.dictation ? "Done" : "Stop"}
        </span>
      ) : state.status === "reconnecting" ? (
        <CircleNotch size={20} className="animate-spin" aria-hidden="true" />
      ) : state.status === "error" ? (
        <WarningCircle size={20} aria-hidden="true" />
      ) : (
        <DancingSticks
          color={colors.accent}
          amplitude={state.amplitude}
          width={26}
          height={20}
          stickWidth={3}
          gap={2}
        />
      )}
    </button>
  );
}

function TranscriptList({
  bubbles,
  colorScheme,
  dictation,
}: {
  dictation: FloatingBarState["dictation"];
  bubbles: FloatingTranscriptBubble[];
  colorScheme: FloatingBarState["colorScheme"];
}) {
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const [pinned, setPinned] = useState(true);

  useEffect(() => {
    if (pinned) {
      bottomRef.current?.scrollIntoView?.({ block: "end" });
    }
  }, [bubbles, pinned, dictation?.text, dictation?.partial]);

  return (
    <div className="relative h-full p-3">
      <div
        className="h-full overflow-y-auto"
        onScroll={(event) => {
          const target = event.currentTarget;
          const distance =
            target.scrollHeight - target.scrollTop - target.clientHeight;
          setPinned(distance < 20);
        }}
      >
        <div
          className={cn([
            "flex min-h-full flex-col gap-2",
            dictation ? "justify-start" : "justify-end",
          ])}
        >
          {dictation ? (
            <DictationTranscript
              dictation={dictation}
              colorScheme={colorScheme}
            />
          ) : (
            bubbles.map((bubble, index) => (
              <TranscriptBubble
                key={bubble.id}
                bubble={bubble}
                colorScheme={colorScheme}
                showsSpeakerLabel={
                  index === 0 ||
                  bubbles[index - 1]?.speakerLabel !== bubble.speakerLabel ||
                  bubbles[index - 1]?.isSelf !== bubble.isSelf
                }
              />
            ))
          )}
          <div ref={bottomRef} />
        </div>
      </div>
      {!pinned &&
      (dictation
        ? !!(dictation.text || dictation.partial)
        : bubbles.length > 0) ? (
        <button
          type="button"
          data-tauri-drag-region="false"
          onClick={() => {
            setPinned(true);
            bottomRef.current?.scrollIntoView?.({
              block: "end",
              behavior: "smooth",
            });
          }}
          className="absolute bottom-3 left-1/2 flex -translate-x-1/2 items-center gap-1.5 rounded-[10px] px-3 py-1.5 text-[11px] font-medium"
          style={{
            background:
              colorScheme === "dark" ? "rgb(46, 46, 43)" : "rgb(242, 242, 237)",
            color: colorScheme === "dark" ? "white" : "rgb(31, 28, 26)",
          }}
        >
          <CaretDown size={10} weight="bold" />
          Go to bottom
        </button>
      ) : null}
    </div>
  );
}

function TranscriptBubble({
  bubble,
  showsSpeakerLabel,
  colorScheme,
}: {
  bubble: FloatingTranscriptBubble;
  showsSpeakerLabel: boolean;
  colorScheme: FloatingBarState["colorScheme"];
}) {
  const overlapping = bubble.overlapsPrevious || bubble.overlapsNext;

  return (
    <div
      className={cn(["flex", bubble.isSelf ? "justify-end" : "justify-start"])}
    >
      <div className="max-w-[calc(100%-40px)] text-left">
        {(showsSpeakerLabel || overlapping) && (
          <p className="mb-1 text-[10px] font-semibold text-white">
            {showsSpeakerLabel ? bubble.speakerLabel : ""}
          </p>
        )}
        <p
          className="rounded-[11px] px-2.5 py-2 text-[13px] leading-5 text-white"
          style={{
            background: bubble.isSelf
              ? `rgba(0, 0, 0, ${colorScheme === "dark" ? 0.34 : 0.24})`
              : `rgba(0, 0, 0, ${colorScheme === "dark" ? 0.28 : 0.2})`,
            boxShadow: overlapping
              ? `inset 0 0 0 1px rgba(255, 255, 255, ${
                  colorScheme === "dark" ? 0.26 : 0.34
                })`
              : undefined,
          }}
        >
          {bubble.text}
        </p>
      </div>
    </div>
  );
}

function HoverHandle({ color }: { color: string }) {
  return (
    <div
      data-tauri-drag-region
      className="flex items-center justify-center"
      style={{
        height: FLOATING_BAR_HOVER_HANDLE_HEIGHT,
        width: "100%",
        paddingTop: FLOATING_BAR_HOVER_HANDLE_TOP_PADDING,
        boxSizing: "content-box",
      }}
    >
      <div
        data-tauri-drag-region
        className="h-full"
        style={{
          width: "calc(100% - 16px)",
          backgroundImage: `radial-gradient(circle, ${color} 0.8px, transparent 0.9px)`,
          backgroundSize: "5px 7px",
        }}
      />
    </div>
  );
}

type BarColors = {
  surface: string;
  envelopeSurface: string;
  content: string;
  handle: string;
  outerStroke: string;
  controlFill: string;
  accent: string;
};

function barColors(state: FloatingBarState): BarColors {
  const opacity = Math.min(Math.max(state.opacity, 0.35), 0.95);
  const dark = state.colorScheme === "dark";
  const content = dark ? "rgb(255, 255, 255)" : "rgb(31, 28, 26)";
  const surfaceRgb = dark ? "110, 112, 102" : "219, 217, 209";

  return {
    surface: `rgba(${surfaceRgb}, ${opacity * 0.82})`,
    envelopeSurface: `rgba(${surfaceRgb}, ${Math.min(opacity * 1.08, 0.95)})`,
    content,
    handle: dark ? "rgba(255, 255, 255, 0.48)" : "rgba(31, 28, 26, 0.36)",
    outerStroke: dark ? "rgba(255, 255, 255, 0.14)" : "rgba(31, 28, 26, 0.12)",
    controlFill: dark ? "rgba(255, 255, 255, 0.08)" : "rgba(31, 28, 26, 0.07)",
    accent: state.status === "error" ? "rgb(255, 64, 61)" : "rgb(255, 51, 77)",
  };
}
