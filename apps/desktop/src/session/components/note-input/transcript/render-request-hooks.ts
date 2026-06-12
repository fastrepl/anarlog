import { useCallback, useMemo, useRef, useSyncExternalStore } from "react";

import type {
  RenderTranscriptHuman,
  RenderTranscriptRequest,
} from "@hypr/plugin-transcription";

import * as main from "~/store/tinybase/store/main";
import {
  buildRenderTranscriptRequestFromRows,
  collectAssignedHumanIdsFromTranscriptRows,
  type RenderTranscriptRequestHumans,
  type TranscriptRow,
} from "~/stt/render-transcript";
import { parseTranscriptHints, parseTranscriptWords } from "~/stt/utils";

type RenderTableId = "transcripts" | "mapping_session_participant" | "humans";
type UiStore = NonNullable<ReturnType<typeof main.UI.useStore>>;

export type TranscriptRowWithId = {
  transcriptId: string;
  row: TranscriptRow;
};

export function useTranscriptRenderData(transcriptId: string): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const sessionId = main.UI.useCell(
    "transcripts",
    transcriptId,
    "session_id",
    main.STORE_ID,
  );

  return useRenderData(sessionId ?? "", [transcriptId]);
}

export function useSessionTranscriptRenderData(sessionId: string): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
      main.STORE_ID,
    ) ?? emptyIds;

  return useRenderData(sessionId, transcriptIds);
}

export function useTranscriptRowsRevision(rowIds: readonly string[]): number {
  const store = main.UI.useStore(main.STORE_ID);

  return useStoreRowsRevision(store, "transcripts", rowIds);
}

function useRenderData(
  sessionId: string,
  transcriptIds: readonly string[],
): {
  request: RenderTranscriptRequest | null;
  transcriptRows: TranscriptRowWithId[];
} {
  const store = main.UI.useStore(main.STORE_ID);
  const selfHumanId = main.UI.useValue("user_id", main.STORE_ID);
  const participantMappingIds =
    main.UI.useSliceRowIds(
      main.INDEXES.sessionParticipantsBySession,
      sessionId,
      main.STORE_ID,
    ) ?? emptyIds;

  const transcriptRowsRevision = useStoreRowsRevision(
    store,
    "transcripts",
    transcriptIds,
  );
  const participantRowsRevision = useStoreRowsRevision(
    store,
    "mapping_session_participant",
    participantMappingIds,
  );

  const transcriptIdsKey = getRowIdsKey(transcriptIds);
  const participantMappingIdsKey = getRowIdsKey(participantMappingIds);

  const transcriptRows = useMemo(() => {
    if (!store || transcriptIds.length === 0) {
      return [];
    }

    return transcriptIds.map((transcriptId) => ({
      transcriptId,
      row: getTranscriptRow(store, transcriptId),
    }));
  }, [store, transcriptIdsKey, transcriptRowsRevision]);

  const participantHumanIds = useMemo(() => {
    if (!store || participantMappingIds.length === 0) {
      return [];
    }

    return collectParticipantHumanIds(store, participantMappingIds);
  }, [store, participantMappingIdsKey, participantRowsRevision]);

  const assignedHumanIds = useMemo(
    () =>
      collectAssignedHumanIdsFromTranscriptRows(
        transcriptRows.map((transcriptRow) => transcriptRow.row),
      ),
    [transcriptRows],
  );

  const humanIds = useMemo(
    () =>
      getUniqueRowIds([
        ...participantHumanIds,
        ...assignedHumanIds,
        typeof selfHumanId === "string" ? selfHumanId : "",
      ]),
    [assignedHumanIds, participantHumanIds, selfHumanId],
  );
  const humanIdsKey = getRowIdsKey(humanIds);
  const humanRowsRevision = useStoreRowsRevision(store, "humans", humanIds);

  const humans = useMemo(() => {
    if (!store) {
      return undefined;
    }

    return collectRenderHumans(store, humanIds, selfHumanId);
  }, [store, humanIdsKey, humanRowsRevision, selfHumanId]);

  const request = useMemo(
    () =>
      buildRenderTranscriptRequestFromRows(
        transcriptRows.map((transcriptRow) => transcriptRow.row),
        humans,
        participantHumanIds,
      ),
    [humans, participantHumanIds, transcriptRows],
  );

  return { request, transcriptRows };
}

function useStoreRowsRevision(
  store: UiStore | undefined,
  tableId: RenderTableId,
  rowIds: readonly string[],
): number {
  const revisionRef = useRef(0);
  const rowIdsKey = getRowIdsKey(rowIds);
  const subscribedRowIds = useMemo(() => getUniqueRowIds(rowIds), [rowIdsKey]);

  const subscribe = useCallback(
    (notify: () => void) => {
      if (!store || subscribedRowIds.length === 0) {
        return noop;
      }

      const listenerIds = subscribedRowIds.map((rowId) =>
        store.addRowListener(tableId, rowId, () => {
          revisionRef.current += 1;
          notify();
        }),
      );

      return () => {
        for (const listenerId of listenerIds) {
          store.delListener(listenerId);
        }
      };
    },
    [store, subscribedRowIds, tableId],
  );
  const getSnapshot = useCallback(() => revisionRef.current, []);

  return useSyncExternalStore(subscribe, getSnapshot, getZero);
}

function getTranscriptRow(store: UiStore, transcriptId: string): TranscriptRow {
  const startedAt = store.getCell("transcripts", transcriptId, "started_at");

  return {
    started_at: typeof startedAt === "number" ? startedAt : null,
    words: parseTranscriptWords(store, transcriptId),
    speaker_hints: parseTranscriptHints(store, transcriptId),
  };
}

function collectParticipantHumanIds(
  store: Pick<UiStore, "getCell">,
  participantMappingIds: readonly string[],
): string[] {
  const humanIds: string[] = [];

  for (const mappingId of participantMappingIds) {
    const humanId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "human_id",
    );

    if (typeof humanId === "string" && humanId) {
      humanIds.push(humanId);
    }
  }

  return getUniqueRowIds(humanIds);
}

function collectRenderHumans(
  store: Pick<UiStore, "getRow">,
  humanIds: readonly string[],
  selfHumanId: unknown,
): RenderTranscriptRequestHumans {
  const humans: RenderTranscriptHuman[] = [];

  for (const humanId of humanIds) {
    const row = store.getRow("humans", humanId);
    if (typeof row.name !== "string" || !row.name) {
      continue;
    }

    humans.push({ human_id: humanId, name: row.name });
  }

  return {
    selfHumanId: typeof selfHumanId === "string" ? selfHumanId : undefined,
    humans,
  };
}

function getRowIdsKey(rowIds: readonly string[]): string {
  return getUniqueRowIds(rowIds).join("\u0000");
}

function getUniqueRowIds(rowIds: readonly string[]): string[] {
  const uniqueRowIds: string[] = [];
  const seen = new Set<string>();

  for (const rowId of rowIds) {
    if (!rowId || seen.has(rowId)) {
      continue;
    }

    uniqueRowIds.push(rowId);
    seen.add(rowId);
  }

  return uniqueRowIds;
}

function noop() {}

function getZero() {
  return 0;
}

const emptyIds: string[] = [];
