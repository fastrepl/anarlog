import { useCallback, useMemo } from "react";

import {
  useMainIndexesInternal,
  useMainStoreInternal,
} from "~/session/hooks/internal";
import * as main from "~/store/tinybase/store/main";
import { updateTranscriptHints, parseTranscriptHints } from "~/stt/utils";

export function useSessionParticipantMappingIds(sessionId: string): string[] {
  return main.UI.useSliceRowIds(
    main.INDEXES.sessionParticipantsBySession,
    sessionId,
    main.STORE_ID,
  );
}

export type ParticipantDetails = {
  mappingId: string;
  sessionId: string;
  humanId: string;
  humanName?: string;
  humanEmail?: string;
  humanJobTitle?: string;
  humanLinkedinUsername?: string;
  orgId?: string;
  orgName?: string;
};

export type SessionParticipantSummary = {
  mappingId: string;
  sessionId: string;
  humanId: string;
  source: string | undefined;
  humanName: string | undefined;
  humanEmail: string | undefined;
  humanJobTitle: string | undefined;
  humanLinkedinUsername: string | undefined;
  orgId: string | undefined;
  orgName: string | undefined;
};

type HumanSearchCandidate = {
  id: string;
  name: string;
  email: string;
  orgId: string | undefined;
  jobTitle: string | undefined;
};

export function useSessionParticipantDetails(
  mappingId: string,
): ParticipantDetails | null {
  const row = main.UI.useResultRow(
    main.QUERIES.sessionParticipantsWithDetails,
    mappingId,
    main.STORE_ID,
  );
  return useMemo(() => {
    if (!row || Object.keys(row).length === 0) return null;
    return {
      mappingId,
      sessionId: (row.session_id as string) ?? "",
      humanId: (row.human_id as string) ?? "",
      humanName: row.human_name as string | undefined,
      humanEmail: row.human_email as string | undefined,
      humanJobTitle: row.human_job_title as string | undefined,
      humanLinkedinUsername: row.human_linkedin_username as string | undefined,
      orgId: row.org_id as string | undefined,
      orgName: row.org_name as string | undefined,
    };
  }, [mappingId, row]);
}

export function useSessionParticipants(
  sessionId: string,
): SessionParticipantSummary[] {
  const mappingIds = useSessionParticipantMappingIds(sessionId);
  const mappingTable = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const participantRows = main.UI.useResultTable(
    main.QUERIES.sessionParticipantsWithDetails,
    main.STORE_ID,
  );

  return useMemo(() => {
    return mappingIds
      .map((mappingId) => {
        const details = participantRows[mappingId];
        if (!details) return null;

        const humanId = details.human_id as string | undefined;
        const mappedSessionId = details.session_id as string | undefined;
        if (!humanId || mappedSessionId !== sessionId) return null;

        const mappingRow = mappingTable[mappingId];
        return {
          mappingId,
          sessionId: mappedSessionId,
          humanId,
          source: mappingRow?.source as string | undefined,
          humanName: details.human_name as string | undefined,
          humanEmail: details.human_email as string | undefined,
          humanJobTitle: details.human_job_title as string | undefined,
          humanLinkedinUsername: details.human_linkedin_username as
            | string
            | undefined,
          orgId: details.org_id as string | undefined,
          orgName: details.org_name as string | undefined,
        } satisfies SessionParticipantSummary;
      })
      .filter((row): row is SessionParticipantSummary => row !== null);
  }, [mappingIds, mappingTable, participantRows, sessionId]);
}

export function useSessionParticipantHumanIds(sessionId: string): string[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(
    () =>
      participants
        .filter((participant) => participant.source !== "excluded")
        .map((participant) => participant.humanId),
    [participants],
  );
}

export function useSessionParticipantNames(sessionId: string): string[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(
    () =>
      participants
        .filter((participant) => participant.source !== "excluded")
        .map((participant) => participant.humanName?.trim() ?? "")
        .filter(Boolean),
    [participants],
  );
}

export function useSessionParticipantPeople(
  sessionId: string,
): { id: string; name: string }[] {
  const participants = useSessionParticipants(sessionId);
  return useMemo(() => {
    return participants
      .filter((participant) => participant.source !== "excluded")
      .map((participant) => ({
        id: participant.humanId,
        name: participant.humanName?.trim() || "",
      }))
      .filter((participant) => participant.id.length > 0);
  }, [participants]);
}

export function useSearchableHumans(
  inputValue: string,
  excludedHumanIds: Set<string>,
): HumanSearchCandidate[] {
  const humans = main.UI.useTable("humans", main.STORE_ID);

  return useMemo(() => {
    const searchLower = inputValue.trim().toLowerCase();

    return Object.entries(humans)
      .filter(([humanId]) => !excludedHumanIds.has(humanId))
      .map(([humanId, row]) => {
        const name = String(row.name ?? "");
        const email = String(row.email ?? "");

        if (
          searchLower &&
          !name.toLowerCase().includes(searchLower) &&
          !email.toLowerCase().includes(searchLower)
        ) {
          return null;
        }

        return {
          id: humanId,
          name,
          email,
          orgId:
            typeof row.org_id === "string" && row.org_id
              ? row.org_id
              : undefined,
          jobTitle:
            typeof row.job_title === "string" && row.job_title
              ? row.job_title
              : undefined,
        } satisfies HumanSearchCandidate;
      })
      .filter((human): human is HumanSearchCandidate => human !== null);
  }, [humans, inputValue, excludedHumanIds]);
}

export function useParticipantSourceCell(mappingId: string): string {
  const v = main.UI.useCell(
    "mapping_session_participant",
    mappingId,
    "source",
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

export function useAllHumanIds(): string[] {
  return main.UI.useRowIds("humans", main.STORE_ID);
}

export function useSessionParticipantMutations(): {
  addParticipant: (args: {
    sessionId: string;
    humanId: string;
    source: string;
  }) => string;
  deleteMapping: (mappingId: string) => void;
  updateMappingHumanId: (mappingId: string, humanId: string) => void;
  createHuman: (args: { id: string; name: string; email: string }) => void;
} {
  const store = useMainStoreInternal();

  const addParticipant = useCallback(
    ({
      sessionId,
      humanId,
      source,
    }: {
      sessionId: string;
      humanId: string;
      source: string;
    }) => {
      if (!store) return "";
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      if (!userId) return "";
      const mappingId = crypto.randomUUID();
      store.setRow("mapping_session_participant", mappingId, {
        user_id: userId,
        session_id: sessionId,
        human_id: humanId,
        source,
      });
      return mappingId;
    },
    [store],
  );

  const deleteMapping = useCallback(
    (mappingId: string) => {
      if (!store) return;
      store.delRow("mapping_session_participant", mappingId);
    },
    [store],
  );

  const updateMappingHumanId = useCallback(
    (mappingId: string, humanId: string) => {
      if (!store) return;
      store.setPartialRow("mapping_session_participant", mappingId, {
        human_id: humanId,
      });
    },
    [store],
  );

  const createHuman = useCallback(
    ({ id, name, email }: { id: string; name: string; email: string }) => {
      if (!store) return;
      const userId = (store.getValue("user_id") as string | undefined) ?? "";
      if (!userId) return;
      store.setRow("humans", id, {
        user_id: userId,
        created_at: new Date().toISOString(),
        name,
        email,
        org_id: "",
        job_title: "",
        linkedin_username: "",
        memo: "",
      });
    },
    [store],
  );

  return { addParticipant, deleteMapping, updateMappingHumanId, createHuman };
}

function parseHumanIdFromHintValue(value: unknown): string | undefined {
  let data = value;
  if (typeof value === "string") {
    try {
      data = JSON.parse(value);
    } catch {
      return undefined;
    }
  }

  if (data && typeof data === "object" && "human_id" in data) {
    const humanId = (data as Record<string, unknown>).human_id;
    return typeof humanId === "string" ? humanId : undefined;
  }

  return undefined;
}

export function useRemoveSessionParticipant(): (args: {
  mappingId: string;
  assignedHumanId?: string;
  sessionId?: string;
  source?: string;
}) => void {
  const store = useMainStoreInternal();
  const indexes = useMainIndexesInternal();

  return useCallback(
    ({ mappingId, assignedHumanId, sessionId, source }) => {
      if (!store) {
        return;
      }

      if (assignedHumanId && sessionId && indexes) {
        const transcriptIds = indexes.getSliceRowIds(
          main.INDEXES.transcriptBySession,
          sessionId,
        );

        for (const transcriptId of transcriptIds) {
          const hints = parseTranscriptHints(store, transcriptId);
          if (hints.length === 0) continue;

          const filteredHints = hints.filter((hint) => {
            if (hint.type !== "user_speaker_assignment") {
              return true;
            }
            const hintHumanId = parseHumanIdFromHintValue(hint.value);
            return hintHumanId !== assignedHumanId;
          });

          if (filteredHints.length !== hints.length) {
            updateTranscriptHints(store, transcriptId, filteredHints);
          }
        }
      }

      if (source === "auto") {
        store.setPartialRow("mapping_session_participant", mappingId, {
          source: "excluded",
        });
      } else {
        store.delRow("mapping_session_participant", mappingId);
      }
    },
    [indexes, store],
  );
}
