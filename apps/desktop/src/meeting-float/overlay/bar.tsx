import {
  ArrowsInSimple,
  ArrowsOutSimple,
  CaretDown,
  Square,
  Warning,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef, useState } from "react";

import type {
  FloatingBarState,
  FloatingTranscriptBubble,
} from "@anlg/plugin-windows";
import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

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
      {...mergeStyleXProps(styles.root, undefined, {
        padding: FLOATING_BAR_INSET,
      })}
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
      {...mergeStyleXProps(styles.panel, undefined, {
        width,
        height,
        borderRadius: FLOATING_BAR_COMPACT_RADIUS,
        background: hovered ? colors.envelopeSurface : colors.surface,
        boxShadow: `inset 0 0 0 0.5px ${colors.outerStroke}`,
      })}
    >
      {hovered ? <HoverHandle color={colors.handle} width={width} /> : null}
      <div
        {...mergeStyleXProps(styles.compactContent, undefined, {
          width,
          height: FLOATING_BAR_COMPACT_HEIGHT,
        })}
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
      {...mergeStyleXProps(styles.panel, undefined, {
        width: FLOATING_BAR_EXPANDED_WIDTH,
        height:
          FLOATING_BAR_EXPANDED_HEIGHT +
          (hovered ? FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT : 0),
        borderRadius: FLOATING_BAR_EXPANDED_RADIUS,
        background: colors.surface,
        boxShadow: `inset 0 0 0 0.5px ${colors.outerStroke}`,
      })}
    >
      <div
        {...mergeStyleXProps(styles.hoverHandleSlot, undefined, {
          height: FLOATING_BAR_HOVER_HANDLE_RESERVED_HEIGHT,
          paddingTop: FLOATING_BAR_HOVER_HANDLE_TOP_PADDING,
          opacity: hovered ? 1 : 0,
        })}
      >
        <HoverHandle
          color={colors.handle}
          width={FLOATING_BAR_EXPANDED_WIDTH}
        />
      </div>
      <div
        {...mergeStyleXProps(styles.expandedContent, undefined, {
          height: FLOATING_BAR_EXPANDED_HEIGHT,
        })}
      >
        <div
          {...mergeStyleXProps(styles.titleRow, undefined, {
            height: FLOATING_BAR_COMPACT_HEIGHT,
            paddingLeft: 16,
            paddingRight:
              compactControlsWidth(state.liveCaptionToggleVisible) + 12,
          })}
        >
          <p
            {...mergeStyleXProps(styles.title, undefined, {
              color: colors.content,
            })}
          >
            {state.title}
          </p>
        </div>
        <TranscriptList
          bubbles={state.transcriptBubbles ?? []}
          colorScheme={state.colorScheme}
        />
        <div
          {...mergeStyleXProps(styles.expandedControls, undefined, {
            width: compactControlsWidth(state.liveCaptionToggleVisible),
            height: FLOATING_BAR_COMPACT_HEIGHT,
            marginRight: FLOATING_BAR_COMPACT_HORIZONTAL_PADDING,
          })}
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
      {...mergeStyleXProps(styles.controls, undefined, {
        gap: FLOATING_BAR_COMPACT_GAP,
      })}
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
          {...mergeStyleXProps(styles.control, undefined, {
            width: FLOATING_BAR_COMPACT_ICON_SIZE,
            height: FLOATING_BAR_COMPACT_ICON_SIZE,
            borderRadius: FLOATING_BAR_CONTROL_RADIUS,
            color: colors.content,
          })}
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
      {...mergeStyleXProps(styles.control, undefined, {
        width,
        height: FLOATING_BAR_COMPACT_ICON_SIZE,
        borderRadius: FLOATING_BAR_CONTROL_RADIUS,
        background: hovered ? "rgba(255, 51, 77, 0.18)" : colors.controlFill,
        color: colors.accent,
      })}
    >
      {hovered ? (
        <span {...stylex.props(styles.stopLabel)}>
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
    <div {...stylex.props(styles.transcript)}>
      <div
        {...stylex.props(styles.transcriptScroll)}
        onScroll={(event) => {
          const target = event.currentTarget;
          const distance =
            target.scrollHeight - target.scrollTop - target.clientHeight;
          setPinned(distance < 20);
        }}
      >
        <div {...stylex.props(styles.transcriptList)}>
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
          {...mergeStyleXProps(styles.scrollToBottom, undefined, {
            background:
              colorScheme === "dark" ? "rgb(46, 46, 43)" : "rgb(242, 242, 237)",
            color: colorScheme === "dark" ? "white" : "rgb(31, 28, 26)",
          })}
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
      {...stylex.props(
        styles.bubbleRow,
        bubble.isSelf ? styles.bubbleRowSelf : styles.bubbleRowOther,
      )}
    >
      <div
        {...stylex.props(
          styles.bubbleContent,
          bubble.isSelf ? styles.bubbleContentSelf : styles.bubbleContentOther,
        )}
      >
        {(showsSpeakerLabel || overlapping) && (
          <p {...stylex.props(styles.speakerLabel)}>
            {showsSpeakerLabel ? bubble.speakerLabel : ""}
          </p>
        )}
        <p
          {...mergeStyleXProps(styles.bubble, undefined, {
            background: bubble.isSelf
              ? `rgba(0, 0, 0, ${colorScheme === "dark" ? 0.34 : 0.24})`
              : `rgba(0, 0, 0, ${colorScheme === "dark" ? 0.28 : 0.2})`,
            boxShadow: overlapping
              ? `inset 0 0 0 1px rgba(255, 255, 255, ${
                  colorScheme === "dark" ? 0.26 : 0.34
                })`
              : undefined,
          })}
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
      {...mergeStyleXProps(styles.handle, undefined, {
        height: FLOATING_BAR_HOVER_HANDLE_HEIGHT,
        width,
      })}
    >
      <div
        data-tauri-drag-region
        {...mergeStyleXProps(styles.handleDots, undefined, {
          width: Math.max(0, width - 16),
          backgroundImage: `radial-gradient(circle, ${color} 0.8px, transparent 0.9px)`,
          backgroundSize: "5px 7px",
        })}
      />
    </div>
  );
}

const styles = stylex.create({
  bubble: {
    borderRadius: "11px",
    color: "white",
    fontSize: "13px",
    lineHeight: "1.25rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.625rem",
  },
  bubbleContent: {
    maxWidth: "calc(100% - 40px)",
  },
  bubbleContentOther: {
    alignItems: "flex-start",
  },
  bubbleContentSelf: {
    alignItems: "flex-end",
  },
  bubbleRow: {
    display: "flex",
  },
  bubbleRowOther: {
    justifyContent: "flex-start",
  },
  bubbleRowSelf: {
    justifyContent: "flex-end",
  },
  compactContent: {
    alignItems: "center",
    bottom: 0,
    display: "flex",
    justifyContent: "center",
    position: "absolute",
    right: 0,
  },
  control: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
  },
  controls: {
    alignItems: "center",
    display: "flex",
  },
  expandedContent: {
    bottom: 0,
    left: 0,
    position: "absolute",
    right: 0,
  },
  expandedControls: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
    position: "absolute",
    right: 0,
    top: 0,
  },
  handle: {
    alignItems: "center",
    display: "flex",
    justifyContent: "center",
  },
  handleDots: {
    height: "100%",
  },
  hoverHandleSlot: {
    left: 0,
    position: "absolute",
    right: 0,
    top: 0,
  },
  panel: {
    overflow: "hidden",
    position: "relative",
  },
  root: {
    alignItems: "flex-end",
    display: "flex",
    height: "100%",
    justifyContent: "flex-end",
    width: "100%",
  },
  scrollToBottom: {
    alignItems: "center",
    borderRadius: "10px",
    bottom: "0.75rem",
    display: "flex",
    fontSize: "11px",
    fontWeight: 500,
    gap: "0.375rem",
    left: "50%",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    position: "absolute",
    transform: "translateX(-50%)",
  },
  speakerLabel: {
    color: "white",
    fontSize: "10px",
    fontWeight: 600,
    marginBottom: "0.25rem",
    paddingInline: "0.25rem",
  },
  stopLabel: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 600,
    gap: "0.375rem",
  },
  title: {
    fontSize: "13px",
    fontWeight: 600,
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  titleRow: {
    alignItems: "center",
    display: "flex",
  },
  transcript: {
    height: "calc(100% - 38px)",
    paddingBottom: "0.75rem",
    paddingInline: "0.75rem",
    position: "relative",
  },
  transcriptList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
    justifyContent: "flex-end",
    minHeight: "100%",
  },
  transcriptScroll: {
    height: "100%",
    overflowY: "auto",
  },
});

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
