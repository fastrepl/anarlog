import {
  DEFAULT_FLOATING_OVERLAY_SETTINGS,
  LIVE_CAPTION_MIN_WIDTH,
  type FloatingOverlaySettings,
  type LiveCaptionPosition,
} from "./settings";

import type { ListenerStore } from "~/store/zustand/listener";
import { SegmentKeyUtils, type RenderLabelContext } from "~/stt/live-segment";

export type ListenerState = ReturnType<ListenerStore["getState"]>;
type FloatingBarStatus = "recording" | "error";
type FloatingBarColorScheme = "light" | "dark";

export type FloatingTranscriptBubble = {
  id: string;
  speakerLabel: string;
  text: string;
  isSelf: boolean;
  isFinal: boolean;
  startMs: number;
  endMs: number;
  overlapsPrevious: boolean;
  overlapsNext: boolean;
};

export type FloatingRouteState = {
  sessionId: string;
  title: string;
  amplitude: number;
  status: FloatingBarStatus;
  colorScheme: FloatingBarColorScheme;
  opacity: number;
  liveCaptionOpacity: number;
  liveCaptionWidth: number;
  liveCaptionLineCount: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
  liveCaptionToggleVisible: boolean;
  transcriptBubbles: FloatingTranscriptBubble[];
};

type LiveCaptionRouteState = {
  sessionId: string;
  text: string;
  opacity: number;
  width: number;
  lineCount: number;
  position: LiveCaptionPosition;
  minimized: boolean;
};

const LIVE_CAPTION_HORIZONTAL_PADDING_PX = 32;
const LIVE_CAPTION_AVERAGE_CHARACTER_WIDTH_PX = 7.8;
const FLOATING_TRANSCRIPT_OVERLAP_THRESHOLD_MS = 300;

export function getFloatingRouteState(
  state: ListenerState,
  {
    sessionId,
    colorScheme = "dark",
    settings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
    liveCaptionToggleVisible = false,
    sessionTitle,
    speakerLabelContext,
  }: {
    sessionId?: string;
    colorScheme?: FloatingBarColorScheme;
    settings?: FloatingOverlaySettings;
    liveCaptionToggleVisible?: boolean;
    sessionTitle?: string | null;
    speakerLabelContext?: RenderLabelContext;
  } = {},
): FloatingRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (sessionId && state.live.sessionId !== sessionId) {
    return null;
  }

  return {
    sessionId: state.live.sessionId,
    title: getFloatingTitle(sessionTitle),
    amplitude: Math.min(
      Math.hypot(state.live.amplitude.mic, state.live.amplitude.speaker),
      1,
    ),
    status: state.live.degraded || state.live.lastError ? "error" : "recording",
    colorScheme,
    opacity: settings.floatingBarOpacity,
    liveCaptionOpacity: settings.liveCaptionOpacity,
    liveCaptionWidth: settings.liveCaptionWidth,
    liveCaptionLineCount: settings.liveCaptionLineCount,
    liveCaptionPosition: settings.liveCaptionPosition,
    liveCaptionMinimized: settings.liveCaptionMinimized,
    liveCaptionToggleVisible,
    transcriptBubbles: getFloatingTranscriptBubbles(
      state.liveSegments,
      speakerLabelContext,
    ),
  };
}

function getFloatingTitle(title: string | null | undefined) {
  const normalized = title?.trim();
  return normalized || "Live transcript";
}

export function getFloatingTranscriptBubbles(
  segments: ListenerState["liveSegments"],
  speakerLabelContext?: RenderLabelContext,
): FloatingTranscriptBubble[] {
  const bubbles = segments
    .slice()
    .sort(
      (a, b) =>
        a.start_ms - b.start_ms ||
        a.end_ms - b.end_ms ||
        a.id.localeCompare(b.id),
    )
    .map((segment) => {
      const text = getFloatingSegmentText(segment);
      if (!text) {
        return null;
      }

      return {
        id: segment.id,
        speakerLabel: getFloatingSpeakerLabel(segment.key, speakerLabelContext),
        text,
        isSelf: isFloatingSelfSpeaker(segment.key),
        isFinal: segment.words.every((word) => word.is_final),
        startMs: segment.start_ms,
        endMs: segment.end_ms,
        overlapsPrevious: false,
        overlapsNext: false,
      };
    })
    .filter((bubble): bubble is FloatingTranscriptBubble => bubble !== null);

  return bubbles.map((bubble, index) => ({
    ...bubble,
    overlapsPrevious: bubbles
      .slice(0, index)
      .some((previous) => doFloatingTranscriptBubblesOverlap(previous, bubble)),
    overlapsNext: bubbles
      .slice(index + 1)
      .some((next) => doFloatingTranscriptBubblesOverlap(bubble, next)),
  }));
}

function getFloatingSegmentText(
  segment: ListenerState["liveSegments"][number],
) {
  const wordText = segment.words
    .map((word) => word.text.trim())
    .filter(Boolean)
    .join(" ")
    .replace(/\s+([,.?!;:])/g, "$1");

  return (wordText || segment.text).trim().replace(/\s+/g, " ");
}

function getFloatingSpeakerLabel(
  key: ListenerState["liveSegments"][number]["key"],
  ctx?: RenderLabelContext,
) {
  if (isFloatingSelfSpeaker(key)) {
    return "You";
  }

  if (ctx) {
    return SegmentKeyUtils.renderLabel(key, ctx);
  }

  if (key.speaker_index != null) {
    return `Speaker ${key.speaker_index + 1}`;
  }

  if (key.channel === "RemoteParty") {
    return "Speaker";
  }

  return "Audio";
}

function isFloatingSelfSpeaker(
  key: ListenerState["liveSegments"][number]["key"],
) {
  return key.channel === "DirectMic";
}

function doFloatingTranscriptBubblesOverlap(
  left: FloatingTranscriptBubble,
  right: FloatingTranscriptBubble,
) {
  if (
    left.isSelf === right.isSelf &&
    left.speakerLabel === right.speakerLabel
  ) {
    return false;
  }

  const overlapMs =
    Math.min(left.endMs, right.endMs) - Math.max(left.startMs, right.startMs);
  return overlapMs >= FLOATING_TRANSCRIPT_OVERLAP_THRESHOLD_MS;
}

export function shouldShowFloatingLiveCaptionToggle({
  liveTranscriptionActive,
}: {
  provider?: string | null;
  model?: string | null;
  liveTranscriptionActive: boolean;
}) {
  return liveTranscriptionActive;
}

export function getFloatingLiveCaptionToggleVisible(state: ListenerState) {
  return shouldShowFloatingLiveCaptionToggle({
    liveTranscriptionActive: state.live.liveTranscriptionActive === true,
  });
}

export function getLiveCaptionRouteState(
  state: ListenerState,
  settings: FloatingOverlaySettings = DEFAULT_FLOATING_OVERLAY_SETTINGS,
): LiveCaptionRouteState | null {
  if (state.live.status !== "active") {
    return null;
  }

  if (!state.live.sessionId) {
    return null;
  }

  if (state.live.liveTranscriptionActive !== true) {
    return null;
  }

  if (settings.liveCaptionMinimized) {
    return null;
  }

  const text = getLiveCaptionDisplayText(state.liveCaptionText, settings);

  return {
    sessionId: state.live.sessionId,
    text,
    opacity: settings.liveCaptionOpacity,
    width: settings.liveCaptionWidth,
    lineCount: settings.liveCaptionLineCount,
    position: settings.liveCaptionPosition,
    minimized: settings.liveCaptionMinimized,
  };
}

export function getCurrentFloatingBarColorScheme(): FloatingBarColorScheme {
  if (typeof document === "undefined") {
    return "dark";
  }

  return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

export function isSameFloatingRouteState(
  left: FloatingRouteState | null,
  right: FloatingRouteState | null,
) {
  return (
    left?.sessionId === right?.sessionId &&
    left?.amplitude === right?.amplitude &&
    left?.status === right?.status &&
    left?.colorScheme === right?.colorScheme &&
    left?.opacity === right?.opacity &&
    left?.liveCaptionOpacity === right?.liveCaptionOpacity &&
    left?.liveCaptionWidth === right?.liveCaptionWidth &&
    left?.liveCaptionLineCount === right?.liveCaptionLineCount &&
    left?.liveCaptionPosition === right?.liveCaptionPosition &&
    left?.liveCaptionMinimized === right?.liveCaptionMinimized &&
    left?.liveCaptionToggleVisible === right?.liveCaptionToggleVisible &&
    left?.title === right?.title &&
    isSameFloatingTranscriptBubbles(
      left?.transcriptBubbles,
      right?.transcriptBubbles,
    )
  );
}

function isSameFloatingTranscriptBubbles(
  left: FloatingTranscriptBubble[] | undefined,
  right: FloatingTranscriptBubble[] | undefined,
) {
  if (left === right) {
    return true;
  }

  if (!left || !right || left.length !== right.length) {
    return false;
  }

  return left.every((bubble, index) => {
    const other = right[index];
    return (
      other &&
      bubble.id === other.id &&
      bubble.speakerLabel === other.speakerLabel &&
      bubble.text === other.text &&
      bubble.isSelf === other.isSelf &&
      bubble.isFinal === other.isFinal &&
      bubble.startMs === other.startMs &&
      bubble.endMs === other.endMs &&
      bubble.overlapsPrevious === other.overlapsPrevious &&
      bubble.overlapsNext === other.overlapsNext
    );
  });
}

export function getLiveCaptionDisplayText(
  text: string,
  settings: Pick<
    FloatingOverlaySettings,
    "liveCaptionWidth" | "liveCaptionLineCount"
  > = DEFAULT_FLOATING_OVERLAY_SETTINGS,
) {
  const normalizedText = text.trim().replace(/\s+/g, " ");
  if (!normalizedText) {
    return "";
  }

  const contentWidth = Math.max(
    settings.liveCaptionWidth - LIVE_CAPTION_HORIZONTAL_PADDING_PX,
    LIVE_CAPTION_MIN_WIDTH - LIVE_CAPTION_HORIZONTAL_PADDING_PX,
  );
  const charactersPerLine = Math.max(
    12,
    Math.floor(contentWidth / LIVE_CAPTION_AVERAGE_CHARACTER_WIDTH_PX),
  );
  const maxCharacters = Math.max(
    24,
    charactersPerLine * settings.liveCaptionLineCount,
  );

  if (normalizedText.length <= maxCharacters) {
    return normalizedText;
  }

  return `... ${getTextSuffixAtWordBoundary(normalizedText, maxCharacters - 4)}`;
}

function getTextSuffixAtWordBoundary(text: string, maxCharacters: number) {
  const suffix = text.slice(-maxCharacters).trimStart();
  const firstWhitespaceIndex = suffix.search(/\s/);
  if (firstWhitespaceIndex > 0 && firstWhitespaceIndex < suffix.length - 1) {
    return suffix.slice(firstWhitespaceIndex).trimStart();
  }

  return suffix;
}
