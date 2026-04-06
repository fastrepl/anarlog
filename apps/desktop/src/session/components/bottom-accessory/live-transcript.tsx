import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef } from "react";

import { cn } from "@hypr/utils";

import { ExpandToggle } from "./expand-toggle";

import { getSegmentColor } from "~/session/components/note-input/transcript/renderer/utils";
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
  isExpanded = false,
  onToggleExpand,
}: {
  sessionId: string;
  isExpanded?: boolean;
  onToggleExpand?: () => void;
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
    <div className="relative w-full pt-3 select-none">
      {onToggleExpand && (
        <ExpandToggle
          isExpanded={isExpanded}
          onToggle={onToggleExpand}
          label="Live"
        />
      )}

      <div className="rounded-xl bg-neutral-50">
        <div
          className={cn([
            "flex min-h-12 items-center gap-2 p-2",
            "w-full max-w-full",
          ])}
        >
          <div className="min-w-0 flex-1 select-none">
            {previewText ? (
              <p className="truncate text-left text-xs text-neutral-600 [direction:rtl]">
                {previewText}
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
    <div className="grid min-w-0 grid-cols-[92px_1fr] items-start gap-x-3">
      <span
        className="mt-0.5 inline-flex min-h-5 items-center justify-start rounded-full px-2 text-[11px] font-medium whitespace-nowrap"
        style={{
          backgroundColor: `${color}1A`,
          color,
        }}
      >
        {label}
      </span>
      <span className="min-w-0 text-xs leading-5 text-neutral-700">
        {getSegmentText(segment)}
      </span>
    </div>
  );
}
