import { useQuery } from "@tanstack/react-query";
import { ChevronDown, ChevronUp, MicOff } from "lucide-react";
import { useMemo, useRef } from "react";

import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import { SegmentKeyUtils, type Segment } from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";
import {
  SpeakerLabelManager,
  defaultRenderLabelContext,
} from "~/stt/segment/shared";

export function LiveTranscriptFooter({
  sessionId,
  showConsentBanner = false,
  isExpanded = false,
  onToggleExpand,
}: {
  sessionId: string;
  showConsentBanner?: boolean;
  isExpanded?: boolean;
  onToggleExpand?: () => void;
}) {
  const store = main.UI.useStore(main.STORE_ID);
  const segments = useLiveTranscriptSegments(sessionId);
  const { amplitude, muted } = useListener((state) => ({
    amplitude: state.live.amplitude,
    muted: state.live.muted,
  }));
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
  const lastSegment = segments[segments.length - 1];
  const lastSegmentText = lastSegment ? getSegmentText(lastSegment) : null;
  const lastSegmentLabel = lastSegment
    ? SegmentKeyUtils.renderLabel(
        lastSegment.key,
        labelContext,
        speakerLabelManager,
      )
    : null;

  return (
    <div className="relative w-full select-none">
      <button
        type="button"
        onClick={onToggleExpand}
        className={cn([
          "absolute top-0 left-1/2 z-10 -translate-x-1/2 -translate-y-1/2",
          "flex h-6 w-10 items-center justify-center rounded-full",
          "border border-neutral-200 bg-white text-neutral-400 shadow-xs",
          "transition-colors hover:bg-neutral-50 hover:text-neutral-600",
        ])}
        aria-label={isExpanded ? "Collapse transcript" : "Expand transcript"}
      >
        {isExpanded ? <ChevronDown size={14} /> : <ChevronUp size={14} />}
      </button>

      <div className="rounded-xl bg-neutral-50">
        <div
          className={cn(["flex items-center gap-2 p-2", "w-full max-w-full"])}
        >
          <div
            className={cn([
              "flex items-center justify-center",
              "h-8 w-8 shrink-0 rounded-full",
              "border border-neutral-200 bg-white shadow-xs",
            ])}
          >
            {muted ? (
              <MicOff className="size-3.5 text-neutral-400" />
            ) : (
              <DancingSticks
                amplitude={Math.min(
                  Math.hypot(amplitude.mic, amplitude.speaker),
                  1,
                )}
                color="#ef4444"
                height={16}
                width={16}
              />
            )}
          </div>

          <div className="min-w-0 flex-1 select-none">
            {showConsentBanner && segments.length === 0 ? (
              <span className="truncate text-xs text-neutral-400">
                Ask for consent when using Char
              </span>
            ) : lastSegmentText ? (
              <p className="truncate text-xs text-neutral-600">
                <span className="font-medium text-neutral-500">
                  {lastSegmentLabel}
                </span>
                {"  "}
                {lastSegmentText}
              </p>
            ) : (
              <span className="text-xs text-neutral-400">Listening...</span>
            )}
          </div>
        </div>

        {isExpanded && (
          <div
            ref={scrollRef}
            className="flex max-h-[180px] flex-col gap-1 overflow-y-auto border-t border-neutral-200/60 px-3 pt-2 pb-2.5"
          >
            {segments.length === 0 ? (
              <span className="py-4 text-center text-xs text-neutral-400">
                Transcript will appear here as you speak.
              </span>
            ) : (
              segments.map((segment, index) => (
                <div
                  key={getSegmentIdentity(segment, index)}
                  className="grid min-w-0 grid-cols-[auto_1fr] items-baseline gap-x-2"
                >
                  <span className="text-[11px] font-medium whitespace-nowrap text-neutral-400">
                    {SegmentKeyUtils.renderLabel(
                      segment.key,
                      labelContext,
                      speakerLabelManager,
                    )}
                  </span>
                  <span className="text-xs text-neutral-700">
                    {getSegmentText(segment)}
                  </span>
                </div>
              ))
            )}
          </div>
        )}
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

  const { data: renderedSegments = [] } = useQuery({
    queryKey: ["live-transcript-footer-segments", sessionId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
  });

  return useMemo(() => {
    return liveSegments.length > 0 ? liveSegments : renderedSegments;
  }, [liveSegments, renderedSegments]);
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
