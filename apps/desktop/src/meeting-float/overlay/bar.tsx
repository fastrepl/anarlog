import {
  ArrowsInSimple,
  ArrowsOutSimple,
  CaretDown,
  Square,
  Warning,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";

import type {
  FloatingBarState,
  FloatingTranscriptBubble,
} from "@anlg/plugin-windows";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { cn } from "@anlg/utils";

import {
  FLOATING_BAR_COMPACT_GAP,
  FLOATING_BAR_COMPACT_HEIGHT,
  FLOATING_BAR_COMPACT_HORIZONTAL_PADDING,
  FLOATING_BAR_COMPACT_ICON_SIZE,
  FLOATING_BAR_COMPACT_SOLO_STOP_WIDTH,
  FLOATING_BAR_COMPACT_STOP_WIDTH,
  FLOATING_BAR_COMPACT_RADIUS,
  FLOATING_BAR_CONTROL_RADIUS,
  FLOATING_BAR_EXPANDED_HEIGHT,
  FLOATING_BAR_EXPANDED_RADIUS,
  FLOATING_BAR_EXPANDED_WIDTH,
  FLOATING_BAR_HOVER_HANDLE_HEIGHT,
  FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT,
  FLOATING_BAR_HOVER_HANDLE_TOP_PADDING,
  FLOATING_BAR_INSET,
  compactControlsWidth,
  compactWidth,
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

  return (
    <div
      className="flex h-full w-full items-end justify-end"
      style={{ padding: FLOATING_BAR_INSET }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {isExpanded ? (
        <ExpandedPanel
          state={state}
          hovered={hovered}
          onStop={onStop}
          onToggleExpanded={onToggleExpanded}
        />
      ) : (
        <CompactPill
          state={state}
          hovered={hovered}
          onStop={onStop}
          onToggleExpanded={onToggleExpanded}
        />
      )}
    </div>
  );
}

function CompactPill({
  state,
  hovered,
  onStop,
  onToggleExpanded,
}: {
  state: FloatingBarState;
  hovered: boolean;
  onStop: () => void;
  onToggleExpanded: (expanded: boolean) => void;
}) {
  const width = compactWidth(state.liveCaptionToggleVisible);
  const height =
    FLOATING_BAR_COMPACT_HEIGHT +
    (hovered ? FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT : 0);
  const colors = barColors(state);

  return (
    <div
      className="relative overflow-hidden"
      style={{
        width,
        height,
        borderRadius: FLOATING_BAR_COMPACT_RADIUS,
        background: hovered ? colors.envelopeSurface : colors.surface,
        boxShadow: `inset 0 0 0 0.5px ${colors.outerStroke}`,
      }}
    >
      {hovered ? <HoverHandle color={colors.handle} width={width} /> : null}
      <div
        className="absolute right-0 bottom-0 flex items-center justify-center"
        style={{
          width,
          height: FLOATING_BAR_COMPACT_HEIGHT,
        }}
      >
        <FloatingControls
          state={state}
          isExpanded={false}
          colors={colors}
          onStop={onStop}
          onToggleExpanded={onToggleExpanded}
        />
      </div>
    </div>
  );
}

function ExpandedPanel({
  state,
  hovered,
  onStop,
  onToggleExpanded,
}: {
  state: FloatingBarState;
  hovered: boolean;
  onStop: () => void;
  onToggleExpanded: (expanded: boolean) => void;
}) {
  const colors = barColors(state);

  return (
    <div
      className="relative overflow-hidden"
      style={{
        width: FLOATING_BAR_EXPANDED_WIDTH,
        height:
          FLOATING_BAR_EXPANDED_HEIGHT +
          (hovered ? FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT : 0),
        borderRadius: FLOATING_BAR_EXPANDED_RADIUS,
        background: colors.surface,
        boxShadow: `inset 0 0 0 0.5px ${colors.outerStroke}`,
      }}
    >
      <div
        className="absolute inset-x-0 top-0"
        style={{
          height: FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT,
          paddingTop: FLOATING_BAR_HOVER_HANDLE_TOP_PADDING,
          opacity: hovered ? 1 : 0,
        }}
      >
        <HoverHandle
          color={colors.handle}
          width={FLOATING_BAR_EXPANDED_WIDTH}
        />
      </div>
      <div
        className="absolute inset-x-0 bottom-0"
        style={{ height: FLOATING_BAR_EXPANDED_HEIGHT }}
      >
        <div
          className="flex items-center"
          style={{
            height: FLOATING_BAR_COMPACT_HEIGHT,
            paddingLeft: 16,
            paddingRight:
              compactControlsWidth(state.liveCaptionToggleVisible) + 12,
          }}
        >
          <p
            className="min-w-0 truncate text-[13px] font-semibold"
            style={{ color: colors.content }}
          >
            {state.title}
          </p>
        </div>
        <TranscriptList
          bubbles={state.transcriptBubbles ?? []}
          colorScheme={state.colorScheme}
        />
        <div
          className="absolute top-0 right-0 flex items-center justify-center"
          style={{
            width: compactControlsWidth(state.liveCaptionToggleVisible),
            height: FLOATING_BAR_COMPACT_HEIGHT,
            marginRight: FLOATING_BAR_COMPACT_HORIZONTAL_PADDING,
          }}
        >
          <FloatingControls
            state={state}
            isExpanded
            colors={colors}
            onStop={onStop}
            onToggleExpanded={onToggleExpanded}
          />
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
      aria-label="Stop listening"
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
      {hovered ? (
        <span className="flex items-center gap-1.5 text-xs font-semibold">
          <Square size={9} weight="fill" />
          Stop
        </span>
      ) : state.status === "error" ? (
        <Warning size={16} weight="fill" />
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
}: {
  bubbles: FloatingTranscriptBubble[];
  colorScheme: FloatingBarState["colorScheme"];
}) {
  const bottomRef = useRef<HTMLDivElement | null>(null);
  const [pinned, setPinned] = useState(true);

  useEffect(() => {
    if (pinned) {
      bottomRef.current?.scrollIntoView?.({ block: "end" });
    }
  }, [bubbles, pinned]);

  return (
    <div className="relative h-[calc(100%-38px)] px-3 pb-3">
      <div
        className="h-full overflow-y-auto"
        onScroll={(event) => {
          const target = event.currentTarget;
          const distance =
            target.scrollHeight - target.scrollTop - target.clientHeight;
          setPinned(distance < 20);
        }}
      >
        <div className="flex min-h-full flex-col justify-end gap-2">
          {bubbles.map((bubble, index) => (
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
          ))}
          <div ref={bottomRef} />
        </div>
      </div>
      {!pinned && bubbles.length > 0 ? (
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
      <div
        className={cn([
          "max-w-[calc(100%-40px)]",
          bubble.isSelf ? "items-end" : "items-start",
        ])}
      >
        {(showsSpeakerLabel || overlapping) && (
          <p className="mb-1 px-1 text-[10px] font-semibold text-white">
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

function HoverHandle({ color, width }: { color: string; width: number }) {
  return (
    <div
      data-tauri-drag-region
      className="flex items-center justify-center"
      style={{
        height: FLOATING_BAR_HOVER_HANDLE_HEIGHT,
        width,
      }}
    >
      <div
        data-tauri-drag-region
        className="h-full"
        style={{
          width: Math.max(0, width - 16),
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
