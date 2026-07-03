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

export function useKeywords(sessionId: string) {
  const rawMd = main.UI.useCell("sessions", sessionId, "raw_md", main.STORE_ID);
  const title = main.UI.useCell("sessions", sessionId, "title", main.STORE_ID);
  const eventJson = main.UI.useCell(
    "sessions",
    sessionId,
    "event_json",
    main.STORE_ID,
  );
  const dictionaryTerms = useConfigValue("personalization_dictionary_terms");

  return useMemo(() => {
    return buildKeywords({
      rawMd,
      title,
      eventJson,
      dictionaryTerms,
    });
  }, [dictionaryTerms, eventJson, rawMd, title]);
}

type KeywordStore = {
  getCell: (
    tableId: "sessions",
    rowId: string,
    cellId: "raw_md" | "title" | "event_json",
  ) => unknown;
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
  return buildKeywords({
    rawMd: store.getCell("sessions", sessionId, "raw_md"),
    title: store.getCell("sessions", sessionId, "title"),
    eventJson: store.getCell("sessions", sessionId, "event_json"),
    dictionaryTerms,
  });
}

export function buildKeywords({
  rawMd,
  title,
  eventJson,
  dictionaryTerms,
}: {
  rawMd: unknown;
  title: unknown;
  eventJson: unknown;
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

  return normalizeKeywordList([...dictionaryTerms, ...keywords, ...keyphrases]);
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

const removeCodeBlocks = (text: string): string =>
  text.replace(/```[\s\S]*?```/g, "").replace(/`[^`]+`/g, "");

const extractHashtags = (text: string): string[] =>
  Array.from(text.matchAll(/#([\p{L}\p{N}_]+)/gu), (match) => match[1]).filter(
    Boolean,
  );

const stripMarkdownFormatting = (text: string): string =>
  text.replace(/[#*_~`[\]()]/g, " ");
