import { useQuery } from "@tanstack/react-query";
import { useMemo, useRef } from "react";

import { cn } from "@hypr/utils";

import { TranscriptViewer } from "~/session/components/note-input/transcript/renderer";
import { TranscriptListeningState } from "~/session/components/note-input/transcript/screens/listening";
import { useTranscriptScreen } from "~/session/components/note-input/transcript/state";
import * as main from "~/store/tinybase/store/main";
import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";
import type { Segment } from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export function DuringSessionAccessory({
  sessionId,
  isFinalizing = false,
  isExpanded = false,
}: {
  sessionId: string;
  isFinalizing?: boolean;
  isExpanded?: boolean;
}) {
  if (isFinalizing) {
    return (
      <div className="relative w-full pt-1 select-none">
        <div className="rounded-xl bg-neutral-50">
          <div className="flex min-h-12 items-center gap-2 p-2">
            <div className="min-w-0 flex-1">
              <span className="text-xs text-neutral-400">Finalizing...</span>
            </div>
          </div>
        </div>
      </div>
    );
  }

  return <LiveTranscriptFooter sessionId={sessionId} isExpanded={isExpanded} />;
}

function LiveTranscriptFooter({
  sessionId,
  isExpanded = false,
}: {
  sessionId: string;
  isExpanded?: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const previewSegments = useLivePreviewSegments(sessionId, screen);
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
  const mode =
    captureMode === "live"
      ? { kind: "live" as const }
      : {
          kind: "record_only" as const,
          isFallbackFromLive: captureMode === "fallback_record_only",
        };

  return (
    <div
      className={cn([
        "w-full select-none",
        mode.kind === "live" && !isExpanded && "relative -mt-[6px] pb-1",
      ])}
    >
      {mode.kind === "record_only" ? (
        <div className="rounded-xl bg-neutral-50">
          <RecordOnlyFooter isFallbackFromLive={mode.isFallbackFromLive} />
        </div>
      ) : (
        <LiveTranscriptContent
          isExpanded={isExpanded}
          previewSegments={previewSegments}
          screen={screen}
        />
      )}
    </div>
  );
}

function RecordOnlyFooter({
  isFallbackFromLive,
}: {
  isFallbackFromLive: boolean;
}) {
  return (
    <div className="flex min-h-8 items-center justify-center px-4">
      <p className="text-[11px] leading-none text-neutral-400">
        {isFallbackFromLive
          ? "Live transcription stopped. Transcript will be created after you stop."
          : "Recording only. Transcript will be created after you stop."}
      </p>
    </div>
  );
}

function LiveTranscriptContent({
  isExpanded,
  previewSegments,
  screen,
}: {
  isExpanded: boolean;
  previewSegments: Segment[];
  screen: ReturnType<typeof useTranscriptScreen>;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);

  if (!isExpanded) {
    return (
      <CollapsedFooterMessage
        message={getTranscriptPreview(previewSegments) ?? "Listening..."}
      />
    );
  }

  const transcriptIds = screen.kind === "ready" ? screen.transcriptIds : [];
  const liveSegments = screen.kind === "ready" ? screen.liveSegments : [];

  return (
    <div className="overflow-hidden rounded-b-xl border-x border-b border-neutral-200 bg-white">
      <div className="h-[300px] overflow-hidden px-3 pt-2">
        {screen.kind === "listening" ? (
          <TranscriptListeningState status={screen.status} />
        ) : (
          <TranscriptViewer
            transcriptIds={transcriptIds}
            liveSegments={liveSegments}
            currentActive
            scrollRef={scrollRef}
            enablePlaybackControls={false}
          />
        )}
      </div>
    </div>
  );
}

function useLivePreviewSegments(
  sessionId: string,
  screen: ReturnType<typeof useTranscriptScreen>,
): Segment[] {
  const store = main.UI.useStore(main.STORE_ID);
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
  const participantMappingsTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const selfHumanId = main.UI.useValue("user_id", main.STORE_ID);
  const transcriptIds = screen.kind === "ready" ? screen.transcriptIds : [];
  const liveSegments = screen.kind === "ready" ? screen.liveSegments : [];

  const request = useMemo(() => {
    if (!store || transcriptIds.length === 0 || liveSegments.length > 0) {
      return null;
    }

    return buildRenderTranscriptRequestFromStore(store, transcriptIds);
  }, [
    store,
    transcriptIds,
    liveSegments.length,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);

  const { data: renderedSegments = [] } = useQuery({
    queryKey: ["live-transcript-footer-preview", sessionId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
  });

  return liveSegments.length > 0 ? liveSegments : renderedSegments;
}

function CollapsedFooterMessage({ message }: { message: string }) {
  return (
    <div
      className={cn([
        "flex min-h-8 items-center gap-2 px-2 py-1",
        "w-full max-w-full",
      ])}
    >
      <div className="min-w-0 flex-1 select-none">
        <p className="truncate text-left text-xs text-neutral-600 [direction:rtl]">
          {message}
        </p>
      </div>
    </div>
  );
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
