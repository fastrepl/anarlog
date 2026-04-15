import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import * as main from "~/store/tinybase/store/main";
import type { Segment } from "~/stt/live-segment";
import {
  buildRenderTranscriptRequestFromStore,
  renderTranscriptSegments,
} from "~/stt/render-transcript";

export function useRenderedTranscriptSegments(
  transcriptId?: string,
): Segment[] {
  const store = main.UI.useStore(main.STORE_ID);
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
  const participantMappingsTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const humansTable = main.UI.useTable("humans", main.STORE_ID);
  const selfHumanId = main.UI.useValue("user_id", main.STORE_ID);

  const request = useMemo(() => {
    if (!store || !transcriptId) {
      return null;
    }

    return buildRenderTranscriptRequestFromStore(store, [transcriptId]);
  }, [
    store,
    transcriptId,
    transcriptsTable,
    participantMappingsTable,
    humansTable,
    selfHumanId,
  ]);

  const { data = [] } = useQuery({
    queryKey: ["rendered-transcript-segments", transcriptId, request],
    queryFn: async () => {
      if (!request) {
        return [];
      }

      return renderTranscriptSegments(request);
    },
    enabled: !!request,
  });

  return data;
}

export function useTranscriptOffset(transcriptId?: string): number {
  const store = main.UI.useStore(main.STORE_ID);
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId ?? "",
    "session_id",
    main.STORE_ID,
  );

  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId ?? "",
    main.STORE_ID,
  );

  return useMemo(() => {
    if (!store || !transcriptId) {
      return 0;
    }

    const transcriptStartedAt = store.getCell(
      "transcripts",
      transcriptId,
      "started_at",
    );
    if (typeof transcriptStartedAt !== "number") {
      return 0;
    }

    let earliestStartedAt = Number.POSITIVE_INFINITY;
    for (const currentTranscriptId of transcriptIds ?? []) {
      const startedAt = store.getCell(
        "transcripts",
        currentTranscriptId,
        "started_at",
      );
      if (typeof startedAt === "number" && startedAt < earliestStartedAt) {
        earliestStartedAt = startedAt;
      }
    }

    return Number.isFinite(earliestStartedAt)
      ? transcriptStartedAt - earliestStartedAt
      : 0;
  }, [store, transcriptId, transcriptIds, transcriptsTable]);
}
