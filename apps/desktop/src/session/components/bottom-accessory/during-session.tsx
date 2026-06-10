import { useQuery } from "@tanstack/react-query";
import { useLayoutEffect, useMemo, useRef } from "react";

import { cn } from "@hypr/utils";

import { getSegmentColor } from "~/session/components/note-input/transcript/renderer/utils";
import * as main from "~/store/tinybase/store/main";
import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";
import {
  mergeRenderedAndLiveSegments,
  SegmentKeyUtils,
  type Segment,
} from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
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
  const segments = useLiveTranscriptSegments(sessionId);
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
  labelContext,
  speakerLabelManager,
}: {
  fillHeight: boolean;
  isExpanded: boolean;
  previewText: string | null;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  segments: Segment[];
  labelContext: ReturnType<typeof defaultRenderLabelContext> | undefined;
  speakerLabelManager: SpeakerLabelManager;
}) {
  const scrollKey = getLiveTranscriptScrollKey(segments);
  const shouldPinToBottomRef = useRef(true);

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
        segments.map((segment, index) => (
          <TranscriptSegmentRow
            key={getSegmentIdentity(segment, index)}
            segment={segment}
            label={SegmentKeyUtils.renderLabel(
              segment.key,
              labelContext,
              speakerLabelManager,
            )}
          />
        ))
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

function useLiveTranscriptSegments(sessionId: string): Segment[] {
  const store = main.UI.useStore(main.STORE_ID);
  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
      main.STORE_ID,
    ) ?? [];
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
  const participantMappingsTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const selfHumanId = main.UI.useValue("user_id", main.STORE_ID);
  const liveSegments = useListener((state) => state.liveSegments);

  const request = useMemo(() => {
    if (!store || transcriptIds.length === 0) {
      return null;
    }

    return buildRenderTranscriptRequestFromStore(store, transcriptIds);
  }, [
    store,
    transcriptIds,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);
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
    return mergeRenderedAndLiveSegments(renderedSegments, liveSegments);
  }, [liveSegments, renderedSegments]);
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
  const lastWord = segment.words[segment.words.length - 1];

  if (firstWord?.id && lastWord?.id) {
    return `${firstWord.id}:${lastWord.id}`;
  }

  return `${segment.key.channel}:${segment.key.speaker_index ?? "unknown"}:${firstWord?.start_ms ?? fallbackIndex}:${lastWord?.end_ms ?? fallbackIndex}`;
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
  label,
}: {
  segment: Segment;
  label: string;
}) {
  const color = getSegmentColor(segment.key);

  return (
    <div className="grid min-w-0 grid-cols-[92px_minmax(0,1fr)] items-start gap-x-3">
      <span
        className="sticky top-2.5 z-10 mt-0.5 flex min-h-5 max-w-full min-w-0 items-center justify-start rounded-full px-2 text-[11px] font-medium"
        title={label}
        style={{
          backgroundColor: `${color}1A`,
          color,
        }}
      >
        <span className="min-w-0 truncate">{label}</span>
      </span>
      <span className="text-muted-foreground min-w-0 text-xs leading-5">
        {getSegmentText(segment)}
      </span>
    </div>
  );
}
