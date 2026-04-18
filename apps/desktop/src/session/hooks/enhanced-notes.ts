import { useCallback, useMemo } from "react";

import { json2md, md2json, parseJsonContent } from "~/editor/markdown";
import {
  useMainIndexesInternal,
  useMainStoreInternal,
} from "~/session/hooks/internal";
import * as main from "~/store/tinybase/store/main";

export type SummaryEditCandidate = {
  enhancedNoteId: string;
  title: string;
  templateId?: string;
  position?: number;
};

export function useEnhancedNoteCell(
  enhancedNoteId: string,
  field: "title" | "content" | "template_id" | "position",
): string {
  const v = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    field,
    main.STORE_ID,
  );
  return (v as string | undefined) ?? "";
}

// `error` is read off enhanced_notes in some paths but isn't in the
// schema; treat it as an untyped escape hatch.
export function useEnhancedNoteUntypedCell(
  enhancedNoteId: string,
  field: string,
): string | undefined {
  const v = main.UI.useCell(
    "enhanced_notes",
    enhancedNoteId,
    field as "title",
    main.STORE_ID,
  );
  return v as string | undefined;
}

export function useUpdateEnhancedNoteContent(
  enhancedNoteId: string,
): (content: string) => void {
  return main.UI.useSetPartialRowCallback(
    "enhanced_notes",
    enhancedNoteId,
    (content: string) => ({ content }),
    [],
    main.STORE_ID,
  );
}

export function useDeleteEnhancedNote(): (enhancedNoteId: string) => void {
  const store = useMainStoreInternal();

  return useCallback(
    (enhancedNoteId: string) => {
      if (!store) return;
      store.delRow("enhanced_notes", enhancedNoteId);
    },
    [store],
  );
}

export function useEditTabTitles(
  sessionId: string,
  enhancedNoteId: string,
): {
  sessionTitle: string | null;
  summaryTitle: string | null;
} {
  const sessionTitle = main.UI.useCell(
    "sessions",
    sessionId,
    "title",
    main.STORE_ID,
  );
  const summaryTitle = useEnhancedNoteCell(enhancedNoteId, "title");

  return useMemo(
    () => ({
      sessionTitle:
        typeof sessionTitle === "string" && sessionTitle.trim()
          ? sessionTitle
          : null,
      summaryTitle:
        typeof summaryTitle === "string" && summaryTitle.trim()
          ? summaryTitle
          : null,
    }),
    [sessionTitle, summaryTitle],
  );
}

// Uses imperative store reads so that the returned callback identities stay
// stable across enhanced_notes mutations. Chat tool registration depends on
// this stability; subscribing to the whole table via useTable would churn
// tool registration on every summary streaming tick.
export function useSummaryEditRuntime(): {
  getSummaryCandidates: (sessionId: string) => SummaryEditCandidate[];
  getSummaryMarkdown: (enhancedNoteId: string) => string;
  applySummaryMarkdown: (enhancedNoteId: string, markdown: string) => void;
} {
  const indexes = useMainIndexesInternal();
  const store = useMainStoreInternal();

  const getSummaryCandidates = useCallback(
    (sessionId: string) => {
      if (!indexes || !store) return [];
      return indexes
        .getSliceRowIds(main.INDEXES.enhancedNotesBySession, sessionId)
        .map((enhancedNoteId) => {
          const row = store.getRow("enhanced_notes", enhancedNoteId);
          return {
            enhancedNoteId,
            title:
              typeof row.title === "string" && row.title.trim()
                ? row.title
                : "Summary",
            templateId:
              typeof row.template_id === "string" && row.template_id
                ? row.template_id
                : undefined,
            position:
              typeof row.position === "number" ? row.position : undefined,
          } satisfies SummaryEditCandidate;
        });
    },
    [indexes, store],
  );

  const getSummaryMarkdown = useCallback(
    (enhancedNoteId: string) => {
      const raw = store?.getCell("enhanced_notes", enhancedNoteId, "content");
      return json2md(
        parseJsonContent(typeof raw === "string" ? raw : undefined),
      );
    },
    [store],
  );

  const applySummaryMarkdown = useCallback(
    (enhancedNoteId: string, markdown: string) => {
      if (!store) {
        throw new Error("Summary storage unavailable");
      }
      const json = md2json(markdown);
      store.setPartialRow("enhanced_notes", enhancedNoteId, {
        content: JSON.stringify(json),
      });
    },
    [store],
  );

  return { getSummaryCandidates, getSummaryMarkdown, applySummaryMarkdown };
}
