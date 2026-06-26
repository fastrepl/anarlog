import { tool } from "ai";
import { z } from "zod";

import { json2md, md2json, parseJsonContent } from "@hypr/editor/markdown";

import type { ToolDependencies } from "./types";

import * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;
type Indexes = NonNullable<ReturnType<typeof main.UI.useIndexes>>;

type CorrectionTarget = "summary" | "transcript" | "summary_and_transcript";

type ReplacementResult = {
  text: string;
  count: number;
};

type TranscriptWord = {
  id?: string | null;
  text?: string | null;
  start_ms?: number | null;
  end_ms?: number | null;
  channel?: number | null;
  speaker?: string | null;
  metadata?: unknown;
};

type SummaryChange = {
  enhancedNoteId: string;
  title: string;
  replacements: number;
};

type TranscriptChange = {
  transcriptId: string;
  wordReplacements: number;
  memoReplacements: number;
};

function replaceExact(
  value: string,
  oldText: string,
  newText: string,
): ReplacementResult {
  if (!oldText) {
    return { text: value, count: 0 };
  }

  const parts = value.split(oldText);
  if (parts.length === 1) {
    return { text: value, count: 0 };
  }

  return {
    text: parts.join(newText),
    count: parts.length - 1,
  };
}

function getSummaryTitle(store: Store, enhancedNoteId: string): string {
  const title = store.getCell("enhanced_notes", enhancedNoteId, "title");
  return typeof title === "string" && title.trim() ? title : "Summary";
}

function getSummaryCandidateIds({
  indexes,
  sessionId,
  enhancedNoteId,
}: {
  indexes: Indexes;
  sessionId: string;
  enhancedNoteId?: string;
}): string[] {
  const noteIds = indexes.getSliceRowIds(
    main.INDEXES.enhancedNotesBySession,
    sessionId,
  );

  if (!enhancedNoteId) {
    return noteIds;
  }

  return noteIds.includes(enhancedNoteId) ? [enhancedNoteId] : [];
}

function hasEnhancedNoteInSession({
  indexes,
  sessionId,
  enhancedNoteId,
}: {
  indexes: Indexes;
  sessionId: string;
  enhancedNoteId: string;
}): boolean {
  return indexes
    .getSliceRowIds(main.INDEXES.enhancedNotesBySession, sessionId)
    .includes(enhancedNoteId);
}

function applySummaryCorrection({
  store,
  indexes,
  sessionId,
  enhancedNoteId,
  oldText,
  newText,
}: {
  store: Store;
  indexes: Indexes;
  sessionId: string;
  enhancedNoteId?: string;
  oldText: string;
  newText: string;
}): SummaryChange[] {
  const noteIds = getSummaryCandidateIds({
    indexes,
    sessionId,
    enhancedNoteId,
  });
  const changes: SummaryChange[] = [];

  for (const noteId of noteIds) {
    const raw = store.getCell("enhanced_notes", noteId, "content");
    const currentContent = json2md(
      parseJsonContent(typeof raw === "string" ? raw : undefined),
    );
    const replaced = replaceExact(currentContent, oldText, newText);
    if (replaced.count === 0) {
      continue;
    }

    store.setPartialRow("enhanced_notes", noteId, {
      content: JSON.stringify(md2json(replaced.text)),
    });
    changes.push({
      enhancedNoteId: noteId,
      title: getSummaryTitle(store, noteId),
      replacements: replaced.count,
    });
  }

  return changes;
}

function parseTranscriptWords(value: unknown): TranscriptWord[] {
  if (typeof value !== "string" || !value) {
    return [];
  }

  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? (parsed as TranscriptWord[]) : [];
  } catch {
    return [];
  }
}

function tokenizeReplacement(value: string): string[] {
  return value
    .split(/\s+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function wordRangeMatchesAt(
  words: TranscriptWord[],
  target: string[],
  start: number,
): boolean {
  if (target.length === 0 || start + target.length > words.length) {
    return false;
  }

  return target.every((text, index) => words[start + index].text === text);
}

function buildReplacementWords(
  original: TranscriptWord[],
  newText: string,
): TranscriptWord[] {
  const tokens = tokenizeReplacement(newText);
  if (tokens.length === 0) {
    return [];
  }

  const first = original[0] ?? {};
  const last = original[original.length - 1] ?? first;
  const startMs = typeof first.start_ms === "number" ? first.start_ms : null;
  const endMs = typeof last.end_ms === "number" ? last.end_ms : startMs;
  const duration =
    startMs !== null && endMs !== null ? Math.max(endMs - startMs, 0) : 0;
  const step = tokens.length > 0 ? duration / tokens.length : 0;
  const baseId = typeof first.id === "string" && first.id ? first.id : null;

  return tokens.map((text, index) => {
    const wordStart =
      startMs === null ? first.start_ms : Math.round(startMs + step * index);
    const wordEnd =
      startMs === null
        ? first.end_ms
        : Math.round(startMs + step * (index + 1));

    return {
      ...first,
      id: index === 0 ? first.id : `${baseId ?? "word"}:correction:${index}`,
      text,
      start_ms: wordStart,
      end_ms: wordEnd,
    };
  });
}

function replaceTranscriptWords(
  words: TranscriptWord[],
  oldText: string,
  newText: string,
): { words: TranscriptWord[]; count: number } {
  const target = tokenizeReplacement(oldText);
  const replacementTokens = tokenizeReplacement(newText);
  if (
    target.length === 0 ||
    replacementTokens.length === 0 ||
    target.length > words.length
  ) {
    return { words, count: 0 };
  }

  const nextWords: TranscriptWord[] = [];
  let count = 0;
  for (let index = 0; index < words.length; ) {
    if (wordRangeMatchesAt(words, target, index)) {
      const original = words.slice(index, index + target.length);
      nextWords.push(...buildReplacementWords(original, newText));
      index += target.length;
      count++;
      continue;
    }

    nextWords.push(words[index]);
    index++;
  }

  return count === 0 ? { words, count: 0 } : { words: nextWords, count };
}

function applyTranscriptCorrection({
  store,
  indexes,
  sessionId,
  oldText,
  newText,
}: {
  store: Store;
  indexes: Indexes;
  sessionId: string;
  oldText: string;
  newText: string;
}): TranscriptChange[] {
  if (tokenizeReplacement(newText).length === 0) {
    return [];
  }

  const transcriptIds = indexes.getSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
  );
  const changes: TranscriptChange[] = [];

  for (const transcriptId of transcriptIds) {
    const rawWords = store.getCell("transcripts", transcriptId, "words");
    const words = parseTranscriptWords(rawWords);
    const wordResult = replaceTranscriptWords(words, oldText, newText);

    const rawMemo = store.getCell("transcripts", transcriptId, "memo_md");
    const hasMemo = typeof rawMemo === "string" && rawMemo.length > 0;
    const memoResult = replaceExact(hasMemo ? rawMemo : "", oldText, newText);

    if (wordResult.count === 0 && memoResult.count === 0) {
      continue;
    }
    if (
      words.length > 0 &&
      hasMemo &&
      (wordResult.count === 0 || memoResult.count === 0)
    ) {
      continue;
    }

    if (wordResult.count > 0) {
      store.setCell(
        "transcripts",
        transcriptId,
        "words",
        JSON.stringify(wordResult.words),
      );
    }
    if (memoResult.count > 0) {
      store.setCell("transcripts", transcriptId, "memo_md", memoResult.text);
    }

    changes.push({
      transcriptId,
      wordReplacements: wordResult.count,
      memoReplacements: memoResult.count,
    });
  }

  return changes;
}

function shouldEditSummary(target: CorrectionTarget): boolean {
  return target === "summary" || target === "summary_and_transcript";
}

function shouldEditTranscript(target: CorrectionTarget): boolean {
  return target === "transcript" || target === "summary_and_transcript";
}

export const buildApplySessionCorrectionTool = (
  deps: Pick<
    ToolDependencies,
    "getStore" | "getIndexes" | "getSessionId" | "getEnhancedNoteId"
  >,
) =>
  tool({
    description:
      "Apply an exact correction to a session summary and/or transcript. Use this when the user corrects note content, for example 'it's not X but Y'. Read the note first if you need the exact old text.",
    inputSchema: z.object({
      sessionId: z
        .string()
        .optional()
        .describe("The session ID to edit. Defaults to the current session."),
      target: z
        .enum(["summary", "transcript", "summary_and_transcript"])
        .default("summary_and_transcript")
        .describe("Which session content to correct."),
      enhancedNoteId: z
        .string()
        .optional()
        .describe(
          "Optional summary ID to restrict summary edits. Defaults to summaries in the target session.",
        ),
      oldText: z
        .string()
        .min(1)
        .describe("Exact text currently present in the note or transcript."),
      newText: z.string().describe("Replacement text."),
    }),
    execute: async (params: {
      sessionId?: string;
      target?: CorrectionTarget;
      enhancedNoteId?: string;
      oldText: string;
      newText: string;
    }) => {
      const store = deps.getStore();
      const indexes = deps.getIndexes();
      const sessionId = params.sessionId ?? deps.getSessionId();
      const target = params.target ?? "summary_and_transcript";

      if (!store || !indexes || !sessionId) {
        return {
          status: "error",
          message:
            "No active session selected. Provide sessionId explicitly when calling apply_session_correction.",
        };
      }

      const newText = params.newText.trim();
      if (!newText) {
        return {
          status: "error",
          message: "Replacement text cannot be blank.",
          sessionId,
        };
      }

      const enhancedNoteId =
        params.enhancedNoteId ??
        (params.sessionId ? undefined : deps.getEnhancedNoteId());
      let editSummary = shouldEditSummary(target);
      if (
        editSummary &&
        enhancedNoteId &&
        !hasEnhancedNoteInSession({
          indexes,
          sessionId,
          enhancedNoteId,
        })
      ) {
        if (target === "summary") {
          return {
            status: "error",
            message:
              "The requested summary does not belong to the target session.",
            sessionId,
          };
        }
        editSummary = false;
      }

      const summaryChanges = editSummary
        ? applySummaryCorrection({
            store,
            indexes,
            sessionId,
            enhancedNoteId,
            oldText: params.oldText,
            newText,
          })
        : [];
      const transcriptChanges = shouldEditTranscript(target)
        ? applyTranscriptCorrection({
            store,
            indexes,
            sessionId,
            oldText: params.oldText,
            newText,
          })
        : [];

      if (summaryChanges.length === 0 && transcriptChanges.length === 0) {
        return {
          status: "not_found",
          message:
            "No exact match found. Read the note and call apply_session_correction with the exact current text.",
          sessionId,
        };
      }

      return {
        status: "applied",
        sessionId,
        summaryChanges,
        transcriptChanges,
      };
    },
  });

export const sessionCorrectionTestInternals = {
  applySummaryCorrection,
  applyTranscriptCorrection,
  replaceExact,
  replaceTranscriptWords,
};
