import { Effect, Option, pipe } from "effect";
import type { UnknownException } from "effect/Cause";
import { toString } from "nlcst-to-string";
import { useMemo } from "react";
import retextEnglish from "retext-english";
import type { Keyphrase, Keyword } from "retext-keywords";
import retextKeywords from "retext-keywords";
import retextPos from "retext-pos";
import retextStringify from "retext-stringify";
import { unified } from "unified";
import type { VFile } from "vfile";

import { useConfigValue } from "~/shared/config";
import * as main from "~/store/tinybase/store/main";
import { normalizeKeywordList } from "~/stt/keywords";

const MAX_TRANSCRIPTION_HINTS = 50;

export function useKeywords(sessionId: string) {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  const participantMappings = main.UI.useTable(
    "mapping_session_participant",
    main.STORE_ID,
  );
  const humans = main.UI.useTable("humans", main.STORE_ID);
  const events = main.UI.useTable("events", main.STORE_ID);
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");

  return useMemo(() => {
    return buildKeywords({
      rawMd,
      title,
      eventJson,
      sessionParticipantTerms: getSessionParticipantNamesFromTables(
        participantMappings,
        humans,
        sessionId,
      ),
      eventParticipantTerms: getAttachedEventParticipantNamesFromTable(
        events,
        eventJson,
      ),
      dictionaryTerms,
    });
  }, [
    dictionaryTerms,
    eventJson,
    events,
    humans,
    participantMappings,
    rawMd,
    sessionId,
    title,
  ]);
}

export type KeywordStore = {
  getCell: main.Store["getCell"];
  forEachRow?: main.Store["forEachRow"];
};

export function getSessionKeywords({
  store,
  sessionId,
  dictionaryTerms,
}: {
  store: KeywordStore;
  sessionId: string;
  dictionaryTerms: string[];
}) {
  return getSessionTranscriptionHints({
    store,
    sessionId,
    dictionaryTerms,
  });
}

export function getSessionTranscriptionHints({
  store,
  sessionId,
  dictionaryTerms,
}: {
  store: KeywordStore;
  sessionId: string;
  dictionaryTerms: string[];
}) {
  const eventJson = store.getCell("sessions", sessionId, "event_json");

  return buildKeywords({
    rawMd: store.getCell("sessions", sessionId, "raw_md"),
    title: store.getCell("sessions", sessionId, "title"),
    eventJson,
    sessionParticipantTerms: getSessionParticipantNames(store, sessionId),
    eventParticipantTerms: getAttachedEventParticipantNames(store, eventJson),
    dictionaryTerms,
  });
}

export function buildKeywords({
  rawMd,
  title,
  eventJson,
  sessionParticipantTerms = [],
  eventParticipantTerms = [],
  dictionaryTerms,
}: {
  rawMd: unknown;
  title: unknown;
  eventJson: unknown;
  sessionParticipantTerms?: string[];
  eventParticipantTerms?: string[];
  dictionaryTerms: string[];
}) {
  const sourceText = buildKeywordSourceText({
    rawMd,
    title,
    eventJson,
  });
  const { keywords, keyphrases } =
    sourceText.length > 0
      ? extractKeywordsFromMarkdown(sourceText)
      : { keywords: [], keyphrases: [] };

  return normalizeKeywordList([
    ...sessionParticipantTerms,
    ...eventParticipantTerms,
    ...dictionaryTerms,
    ...keywords,
    ...keyphrases,
  ]).slice(0, MAX_TRANSCRIPTION_HINTS);
}

export function buildKeywordSourceText({
  rawMd,
  title,
  eventJson,
}: {
  rawMd: unknown;
  title: unknown;
  eventJson: unknown;
}): string {
  return [
    stringValue(rawMd),
    stringValue(title),
    ...eventKeywordFields(eventJson),
  ]
    .filter((value) => value.length > 0)
    .join("\n");
}

export const extractKeywordsFromMarkdown = (
  markdown: string,
): { keywords: string[]; keyphrases: string[] } =>
  pipe(
    Effect.succeed(markdown),
    Effect.map(removeCodeBlocks),
    Effect.map((text) => ({
      hashtags: extractHashtags(text),
      cleaned: stripMarkdownFormatting(text),
    })),
    Effect.flatMap(({ cleaned, hashtags }) =>
      cleaned.trim().length === 0
        ? Effect.succeed({ keywords: hashtags, keyphrases: [] })
        : pipe(
            processMarkdown(cleaned),
            Effect.map((file) => gatherKeywords(file, hashtags)),
            Effect.orElse(() =>
              Effect.succeed({
                keywords: hashtags,
                keyphrases: [],
              }),
            ),
          ),
    ),
    Effect.runSync,
  );

const processMarkdown = (
  markdown: string,
): Effect.Effect<VFile, UnknownException, never> =>
  Effect.try(() =>
    unified()
      .use(retextEnglish)
      .use(retextPos)
      .use(retextKeywords, { maximum: 50 })
      .use(retextStringify)
      .processSync(markdown),
  );

const gatherKeywords = (
  file: VFile,
  hashtags: string[],
): { keywords: string[]; keyphrases: string[] } => {
  const keywords = pipe(
    Option.fromNullable(file.data.keywords),
    Option.map((entries) => entries.flatMap(extractKeywordMatches)),
    Option.getOrElse(() => [] as string[]),
  );

  const keyphrases = pipe(
    Option.fromNullable(file.data.keyphrases),
    Option.map((entries) => entries.flatMap(extractKeyphraseMatches)),
    Option.getOrElse(() => [] as string[]),
  );

  return {
    keywords: [...hashtags, ...keywords].filter(
      (keyword) => keyword.length >= 2,
    ),
    keyphrases: keyphrases.filter((phrase) => phrase.length >= 2),
  };
};

const extractKeywordMatches = (keyword: Keyword): string[] =>
  keyword.matches.flatMap((match) => {
    const text = toString(match.node).trim();
    return text.length > 0 ? [text] : [];
  });

const extractKeyphraseMatches = (phrase: Keyphrase): string[] =>
  phrase.matches.flatMap((match) => {
    const text = toString(match.nodes).trim();
    return text.length > 0 ? [text] : [];
  });

const stringValue = (value: unknown): string =>
  typeof value === "string" ? value.trim() : "";

const eventKeywordFields = (eventJson: unknown): string[] => {
  if (typeof eventJson !== "string" || !eventJson) {
    return [];
  }

  try {
    const event = JSON.parse(eventJson);
    return [event?.title, event?.description, event?.location].flatMap(
      (value) => {
        const text = stringValue(value);
        return text ? [text] : [];
      },
    );
  } catch {
    return [];
  }
};

const getSessionParticipantNames = (
  store: KeywordStore,
  sessionId: string,
): string[] => {
  if (!store.forEachRow) {
    return [];
  }

  const names: string[] = [];
  store.forEachRow("mapping_session_participant", (mappingId, _forEachCell) => {
    const mappedSessionId = store.getCell(
      "mapping_session_participant",
      mappingId,
      "session_id",
    );
    const source = store.getCell(
      "mapping_session_participant",
      mappingId,
      "source",
    );
    if (mappedSessionId !== sessionId || source === "excluded") {
      return;
    }

    const humanId = stringValue(
      store.getCell("mapping_session_participant", mappingId, "human_id"),
    );
    if (!humanId) {
      return;
    }

    const name = stringValue(store.getCell("humans", humanId, "name"));
    if (name) {
      names.push(name);
    }
  });

  return names;
};

const getSessionParticipantNamesFromTables = (
  mappings: TableRows,
  humans: TableRows,
  sessionId: string,
): string[] =>
  Object.values(mappings).flatMap((mapping) => {
    if (mapping?.session_id !== sessionId || mapping.source === "excluded") {
      return [];
    }

    const humanId = stringValue(mapping.human_id);
    if (!humanId) {
      return [];
    }

    const name = stringValue(humans[humanId]?.name);
    return name ? [name] : [];
  });

const getAttachedEventParticipantNames = (
  store: KeywordStore,
  eventJson: unknown,
): string[] => {
  if (!store.forEachRow) {
    return [];
  }

  const sessionEvent = parseSessionEvent(eventJson);
  if (!sessionEvent) {
    return [];
  }

  let participantsJson: unknown;
  store.forEachRow("events", (eventId, _forEachCell) => {
    if (participantsJson !== undefined) {
      return;
    }

    const trackingId = store.getCell("events", eventId, "tracking_id_event");
    const calendarId = store.getCell("events", eventId, "calendar_id");
    if (
      trackingId === sessionEvent.trackingId &&
      calendarId === sessionEvent.calendarId
    ) {
      participantsJson = store.getCell("events", eventId, "participants_json");
    }
  });

  return parseEventParticipantNames(participantsJson);
};

type TableRows = Record<string, Record<string, unknown> | undefined>;

const getAttachedEventParticipantNamesFromTable = (
  events: TableRows,
  eventJson: unknown,
): string[] => {
  const sessionEvent = parseSessionEvent(eventJson);
  if (!sessionEvent) {
    return [];
  }

  const event = Object.values(events).find(
    (event) =>
      event?.tracking_id_event === sessionEvent.trackingId &&
      event?.calendar_id === sessionEvent.calendarId,
  );

  return parseEventParticipantNames(event?.participants_json);
};

const parseSessionEvent = (
  eventJson: unknown,
): { trackingId: string; calendarId: string } | null => {
  if (typeof eventJson !== "string" || !eventJson) {
    return null;
  }

  try {
    const event = JSON.parse(eventJson);
    const trackingId = stringValue(event?.tracking_id);
    const calendarId = stringValue(event?.calendar_id);
    return trackingId && calendarId ? { trackingId, calendarId } : null;
  } catch {
    return null;
  }
};

const parseEventParticipantNames = (participantsJson: unknown): string[] => {
  if (typeof participantsJson !== "string" || !participantsJson) {
    return [];
  }

  try {
    const participants = JSON.parse(participantsJson);
    if (!Array.isArray(participants)) {
      return [];
    }

    return participants.flatMap((participant) => {
      if (participant?.is_current_user === true) {
        return [];
      }

      const name = stringValue(participant?.name);
      return name ? [name] : [];
    });
  } catch {
    return [];
  }
};

const removeCodeBlocks = (text: string): string =>
  text.replace(/```[\s\S]*?```/g, "").replace(/`[^`]+`/g, "");

const extractHashtags = (text: string): string[] =>
  Array.from(text.matchAll(/#([\p{L}\p{N}_]+)/gu), (match) => match[1]).filter(
    Boolean,
  );

const stripMarkdownFormatting = (text: string): string =>
  text.replace(/[#*_~`[\]()]/g, " ");
