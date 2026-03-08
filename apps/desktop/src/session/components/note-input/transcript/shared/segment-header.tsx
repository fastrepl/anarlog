import { useMemo } from "react";

import type { Segment } from "@hypr/transcript";
import {
  getTimestampRange,
  SegmentHeader as SharedSegmentHeader,
  useSegmentColor,
} from "@hypr/transcript/ui";

import { Operations } from "./operations";

import * as main from "~/store/tinybase/store/main";
import { SegmentKey } from "~/stt/segment";
import {
  defaultRenderLabelContext,
  SpeakerLabelManager,
} from "~/stt/segment/shared";

export function SegmentHeader({
  segment,
  operations,
  sessionId,
  speakerLabelManager,
}: {
  segment: Segment;
  operations?: Operations;
  sessionId?: string;
  speakerLabelManager?: SpeakerLabelManager;
}) {
  const color = useSegmentColor(segment.key);
  const label = useSpeakerLabel(segment.key, speakerLabelManager);
  const participants = useSessionParticipants(sessionId);
  const timestamp = getTimestampRange(segment);

  return (
    <SharedSegmentHeader
      segment={segment}
      speakerInfo={{ label, color }}
      timestamp={timestamp}
      operations={operations}
      participants={participants}
    />
  );
}

function useSpeakerLabel(key: Segment["key"], manager?: SpeakerLabelManager) {
  const store = main.UI.useStore(main.STORE_ID);

  return useMemo(() => {
    if (!store) {
      return SegmentKey.renderLabel(key, undefined, manager);
    }
    const ctx = defaultRenderLabelContext(store);
    return SegmentKey.renderLabel(key, ctx, manager);
  }, [key, store, manager]);
}

function useSessionParticipants(sessionId?: string) {
  const mappingIds = main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId ?? "",
    main.STORE_ID,
  ) as string[];

  const queries = main.UI.useQueries(main.STORE_ID);

  return useMemo(() => {
    if (!queries || !sessionId) {
      return [];
    }

    const participants: Array<{ humanId: string; name: string }> = [];

    for (const mappingId of mappingIds) {
      const result = queries.getResultRow(
        main.QUERIES.sessionParticipantsWithDetails,
        mappingId,
      );

      if (!result) {
        continue;
      }

      const humanId = result.human_id as string;
      const name = (result.human_name as string) || "";

      participants.push({ humanId, name });
    }

    return participants;
  }, [mappingIds, queries, sessionId]);
}
