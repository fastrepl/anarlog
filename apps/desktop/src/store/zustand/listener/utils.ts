import type { RuntimeSpeakerHint, WordLike } from "~/stt/segment";
import {
  createTranscriptTimingMetadata,
  type TranscriptTimingSource,
  type TranscriptWordMetadata,
} from "~/stt/timing";

export function fixSpacingForWords(
  words: string[],
  transcript: string,
): string[] {
  const result: string[] = [];
  let pos = 0;

  for (const [i, word] of words.entries()) {
    const trimmed = word.trim();

    if (!trimmed) {
      result.push(word);
      continue;
    }

    const foundAt = transcript.indexOf(trimmed, pos);
    if (foundAt === -1) {
      result.push(word);
      continue;
    }

    const prefix = i === 0 ? " " : transcript.slice(pos, foundAt);
    result.push(prefix + trimmed);
    pos = foundAt + trimmed.length;
  }

  return result;
}

export type WordEntry = {
  word: string;
  punctuated_word?: string | null;
  start: number;
  end: number;
  channel?: number;
  speaker?: number | null;
  metadata?: TranscriptWordMetadata | null;
};

export function transformWordEntries(
  wordEntries: WordEntry[] | null | undefined,
  transcript: string,
  channel: number,
  options: {
    timingSource?: TranscriptTimingSource;
  } = {},
): [WordLike[], RuntimeSpeakerHint[]] {
  const words: WordLike[] = [];
  const hints: RuntimeSpeakerHint[] = [];

  const entries = wordEntries ?? [];
  const textsWithSpacing = fixSpacingForWords(
    entries.map((w) => w.punctuated_word ?? w.word),
    transcript,
  );

  for (let i = 0; i < entries.length; i++) {
    const word = entries[i];
    const text = textsWithSpacing[i];

    words.push({
      text,
      start_ms: Math.round(word.start * 1000),
      end_ms: Math.round(word.end * 1000),
      channel: typeof word.channel === "number" ? word.channel : channel,
      metadata: createTranscriptTimingMetadata(
        options.timingSource ?? "provider_word",
        word.metadata,
      ),
    });

    if (typeof word.speaker === "number") {
      hints.push({
        wordIndex: i,
        data: {
          type: "provider_speaker_index",
          speaker_index: word.speaker,
        },
      });
    }
  }

  return [words, hints];
}
