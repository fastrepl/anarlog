import { useQuery } from "@tanstack/react-query";
import { useCallback, useLayoutEffect, useMemo, useRef, useState } from "react";

import { cn } from "@hypr/utils";

import { SpeakerAssignPopover } from "../note-input/transcript/renderer/speaker-assign";

import { useSessionTranscriptRenderData } from "~/session/components/note-input/transcript/render-request-hooks";
import { useSegmentColorVars } from "~/session/components/note-input/transcript/renderer/utils";
import * as main from "~/store/tinybase/store/main";
import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";
import {
  mergeRenderedAndLiveSegments,
  SegmentKeyUtils,
  type Segment,
} from "~/stt/live-segment";
import {
  getRenderTranscriptRequestKey,
  renderTranscriptSegments,
} from "~/stt/render-transcript";
import {
  SpeakerLabelManager,
  defaultRenderLabelContext,
} from "~/stt/segment/shared";

export function DuringSessionAccessory({
  sessionId,
  fillHeight = false,
  isExpanded = false,
}: {
  sessionId: string;
  fillHeight?: boolean;
  isExpanded?: boolean;
}) {
  return (
    <LiveTranscriptFooter
      sessionId={sessionId}
      fillHeight={fillHeight}
      isExpanded={isExpanded}
    />
  );
}

function LiveTranscriptFooter({
  sessionId,
  fillHeight,
  isExpanded = false,
}: {
  sessionId: string;
  fillHeight: boolean;
  isExpanded?: boolean;
}) {
  const requestedLiveTranscription = useListener(
    (state) => state.live.requestedLiveTranscription,
  );
  const liveTranscriptionActive = useListener(
    (state) => state.live.liveTranscriptionActive,
  );
  const captureMode = getLiveCaptureUiMode({
    requestedLiveTranscription,
    liveTranscriptionActive,
  });

  if (captureMode !== "live") {
    return null;
  }

  return (
    <LiveTranscriptFooterContent
      sessionId={sessionId}
      fillHeight={fillHeight}
      isExpanded={isExpanded}
    />
  );
}

function LiveTranscriptFooterContent({
  sessionId,
  fillHeight,
  isExpanded = false,
}: {
  sessionId: string;
  fillHeight: boolean;
  isExpanded?: boolean;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const { segments, transcriptIdByWordId } = useLiveTranscriptData(sessionId);
  const labelContext = useMemo(
    () => (store ? defaultRenderLabelContext(store) : undefined),
    [store],
  );

  const speakerLabelManager = useMemo(() => {
    if (!store) {
      return new SpeakerLabelManager();
    }

    return SpeakerLabelManager.fromSegments(segments, labelContext);
  }, [labelContext, segments, store]);

  const scrollRef = useRef<HTMLDivElement>(null);
  const previewText = useMemo(() => getTranscriptPreview(segments), [segments]);

  return (
    <div className={cn(["w-full select-none", fillHeight && "h-full min-h-0"])}>
      <div className={cn(["rounded-xl", fillHeight && "h-full min-h-0"])}>
        <LiveTranscriptContent
          fillHeight={fillHeight}
          isExpanded={isExpanded}
          previewText={previewText}
          scrollRef={scrollRef}
          segments={segments}
          transcriptIdByWordId={transcriptIdByWordId}
          labelContext={labelContext}
          speakerLabelManager={speakerLabelManager}
        />
      </div>
    </div>
  );
}

function LiveTranscriptContent({
  fillHeight,
  isExpanded,
  previewText,
  scrollRef,
  segments,
  transcriptIdByWordId,
  labelContext,
  speakerLabelManager,
}: {
  fillHeight: boolean;
  isExpanded: boolean;
  previewText: string | null;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  segments: Segment[];
  transcriptIdByWordId: Map<string, string>;
  labelContext: ReturnType<typeof defaultRenderLabelContext> | undefined;
  speakerLabelManager: SpeakerLabelManager;
}) {
  const scrollKey = getLiveTranscriptScrollKey(segments);
  const shouldPinToBottomRef = useRef(true);
  const [speakerAssignments, setSpeakerAssignments] = useState(
    () => new Map<string, string>(),
  );

  const handleSpeakerAssigned = useCallback(
    (transcriptId: string, segmentKey: Segment["key"], humanId: string) => {
      setSpeakerAssignments((current) => {
        const next = new Map(current);
        next.set(
          getSpeakerAssignmentStateKey(transcriptId, segmentKey),
          humanId,
        );
        return next;
      });
    },
    [],
  );

  useLayoutEffect(() => {
    if (!isExpanded) {
      shouldPinToBottomRef.current = true;
      return;
    }

    if (shouldPinToBottomRef.current) {
      pinLiveTranscriptToBottom(scrollRef);
    }
  }, [isExpanded, scrollKey, scrollRef]);

  if (!isExpanded) {
    return <CollapsedFooterMessage message={previewText ?? "Listening..."} />;
  }

  return (
    <div
      ref={scrollRef}
      data-live-transcript-scroll
      onScroll={() => {
        const element = scrollRef.current;
        if (!element) {
          return;
        }

        shouldPinToBottomRef.current = isLiveTranscriptPinnedToBottom(element);
      }}
      className={cn([
        "flex flex-col gap-1 overflow-y-auto px-3 py-2.5",
        fillHeight ? "h-full min-h-0" : "max-h-[180px]",
      ])}
    >
      {segments.length === 0 ? (
        <span className="text-muted-foreground py-4 text-center text-xs">
          Transcript will appear here as you speak.
        </span>
      ) : (
        segments.map((segment, index) => {
          const transcriptId = getSegmentTranscriptId(
            segment,
            transcriptIdByWordId,
          );
          const labelKey = getSegmentLabelKey(
            segment.key,
            transcriptId,
            speakerAssignments,
          );

          return (
            <TranscriptSegmentRow
              key={getSegmentIdentity(segment, index)}
              segment={segment}
              transcriptId={transcriptId}
              label={SegmentKeyUtils.renderLabel(
                labelKey,
                labelContext,
                speakerLabelManager,
              )}
              onSpeakerAssigned={handleSpeakerAssigned}
            />
          );
        })
      )}
    </div>
  );
}

function pinLiveTranscriptToBottom(
  scrollRef: React.RefObject<HTMLDivElement | null>,
) {
  const element = scrollRef.current;
  if (!element) {
    return;
  }

  element.scrollTop = element.scrollHeight;
}

const LIVE_TRANSCRIPT_BOTTOM_THRESHOLD_PX = 24;

function isLiveTranscriptPinnedToBottom(element: HTMLDivElement) {
  return (
    element.scrollHeight - element.clientHeight - element.scrollTop <=
    LIVE_TRANSCRIPT_BOTTOM_THRESHOLD_PX
  );
}

function CollapsedFooterMessage({ message }: { message: string }) {
  return (
    <div
      className={cn([
        "flex min-h-7 items-center gap-2 px-2 py-2",
        "w-full max-w-full",
      ])}
    >
      <div className="min-w-0 flex-1 select-none">
        <p className="text-muted-foreground truncate text-left text-xs [direction:rtl]">
          {message}
        </p>
      </div>
    </div>
  );
}

function useLiveTranscriptData(sessionId: string): {
  segments: Segment[];
  transcriptIdByWordId: Map<string, string>;
} {
  const { request, transcriptRows } = useSessionTranscriptRenderData(sessionId);
  const liveSegments = useListener((state) => state.liveSegments);
  const requestKey = useMemo(
    () => getRenderTranscriptRequestKey(request),
    [request],
  );

  const { data: renderedSegments = [] } = useQuery({
    queryKey: ["live-transcript-footer-segments", sessionId, requestKey],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
    gcTime: 0,
  });

  return useMemo(() => {
    const segments = mergeRenderedAndLiveSegments(
      renderedSegments,
      liveSegments,
    );
    const transcriptIdByWordId = new Map<string, string>();

    for (const { transcriptId, row } of transcriptRows) {
      for (const word of row.words ?? []) {
        if (typeof word.id === "string" && word.id) {
          transcriptIdByWordId.set(word.id, transcriptId);
        }
      }
    }

    return { segments, transcriptIdByWordId };
  }, [liveSegments, renderedSegments, transcriptRows]);
}

function getLiveTranscriptScrollKey(segments: Segment[]): string {
  const lastSegment = segments[segments.length - 1];
  const lastWord = lastSegment?.words[lastSegment.words.length - 1];

  if (!lastSegment || !lastWord) {
    return String(segments.length);
  }

  return [
    segments.length,
    lastSegment.words.length,
    getSegmentIdentity(lastSegment, segments.length - 1),
    lastWord.id ?? "",
    lastWord.text,
    lastWord.end_ms,
  ].join(":");
}

function getSegmentIdentity(segment: Segment, fallbackIndex: number): string {
  const firstWord = segment.words[0];
  const serializedKey = SegmentKeyUtils.serialize(segment.key);

  if (firstWord?.id) {
    return `${serializedKey}:${firstWord.id}`;
  }

  if (firstWord) {
    return `${serializedKey}:${firstWord.start_ms}`;
  }

  return `${serializedKey}:${segment.id ?? fallbackIndex}`;
}

function getSegmentText(segment: Segment): string {
  const text = segment.words
    .map((word) => word.text)
    .join("")
    .trim();
  return text || "…";
}

function getTranscriptPreview(segments: Segment[]): string | null {
  const transcript = segments
    .map((segment) =>
      segment.words
        .map((word) => word.text)
        .join("")
        .trim(),
    )
    .filter(Boolean)
    .join(" ")
    .trim();

  if (!transcript) {
    return null;
  }

  return transcript.length > 500 ? transcript.slice(-500) : transcript;
}

function TranscriptSegmentRow({
  segment,
  transcriptId,
  label,
  onSpeakerAssigned,
}: {
  segment: Segment;
  transcriptId: string | undefined;
  label: string;
  onSpeakerAssigned: (
    transcriptId: string,
    segmentKey: Segment["key"],
    humanId: string,
  ) => void;
}) {
  const colorVars = useSegmentColorVars(segment.key);

  return (
    <div className="grid min-w-0 grid-cols-[92px_minmax(0,1fr)] items-start gap-x-3">
      <span
        className="sticky top-2.5 z-10 mt-0.5 flex min-h-5 max-w-full min-w-0 items-center justify-start rounded-full px-2 text-[11px] font-medium [--segment-color:var(--segment-color-light)] dark:[--segment-color:var(--segment-color-dark)]"
        title={label}
        style={{
          ...colorVars,
          backgroundColor:
            "color-mix(in srgb, var(--segment-color) 10%, transparent)",
          color: "var(--segment-color)",
        }}
      >
        {transcriptId ? (
          <SpeakerAssignPopover
            segment={segment}
            transcriptId={transcriptId}
            color="var(--segment-color)"
            label={label}
            className="max-w-full min-w-0 truncate text-left"
            onAssigned={(humanId) =>
              onSpeakerAssigned(transcriptId, segment.key, humanId)
            }
          />
        ) : (
          <span className="min-w-0 truncate">{label}</span>
        )}
      </span>
      <span className="text-muted-foreground min-w-0 text-xs leading-5">
        {getSegmentText(segment)}
      </span>
    </div>
  );
}

function getSegmentTranscriptId(
  segment: Segment,
  transcriptIdByWordId: Map<string, string>,
): string | undefined {
  for (const word of segment.words) {
    if (word.id) {
      const transcriptId = transcriptIdByWordId.get(word.id);
      if (transcriptId) {
        return transcriptId;
      }
    }
  }

  return undefined;
}

function getSegmentLabelKey(
  segmentKey: Segment["key"],
  transcriptId: string | undefined,
  speakerAssignments: Map<string, string>,
): Segment["key"] {
  if (!transcriptId) {
    return segmentKey;
  }

  const humanId = speakerAssignments.get(
    getSpeakerAssignmentStateKey(transcriptId, segmentKey),
  );
  return humanId ? { ...segmentKey, speaker_human_id: humanId } : segmentKey;
}

function getSpeakerAssignmentStateKey(
  transcriptId: string,
  segmentKey: Segment["key"],
): string {
  return `${transcriptId}:${SegmentKeyUtils.serialize(segmentKey)}`;
}
