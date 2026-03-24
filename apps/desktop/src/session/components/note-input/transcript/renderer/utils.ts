import chroma from "chroma-js";
import { useMemo } from "react";

import type { Segment, SegmentKey, SegmentWord } from "~/stt/live-segment";

export type HighlightSegment = { text: string; isMatch: boolean };

export type SentenceLine = {
  words: SegmentWord[];
  startMs: number;
  endMs: number;
};

export function groupWordsIntoLines(words: SegmentWord[]): SentenceLine[] {
  if (words.length === 0) return [];

  const lines: SentenceLine[] = [];
  let currentLine: SegmentWord[] = [];

  for (const word of words) {
    currentLine.push(word);
    const text = word.text.trim();
    if (text.endsWith(".") || text.endsWith("?") || text.endsWith("!")) {
      lines.push({
        words: currentLine,
        startMs: currentLine[0]!.start_ms,
        endMs: currentLine[currentLine.length - 1]!.end_ms,
      });
      currentLine = [];
    }
  }

  if (currentLine.length > 0) {
    lines.push({
      words: currentLine,
      startMs: currentLine[0]!.start_ms,
      endMs: currentLine[currentLine.length - 1]!.end_ms,
    });
  }

  return lines;
}

export function formatTimestamp(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
  }

  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

export function getTimestampRange(segment: Segment): string {
  if (segment.words.length === 0) {
    return "00:00 - 00:00";
  }

  const firstWord = segment.words[0]!;
  const lastWord = segment.words[segment.words.length - 1]!;
  return `${formatTimestamp(firstWord.start_ms)} - ${formatTimestamp(lastWord.end_ms)}`;
}

export function getSegmentColor(key: SegmentKey): string {
  const speakerIndex = key.speaker_index ?? 0;

  const channelPalettes = [
    [10, 25, 0, 340, 15, 350],
    [285, 305, 270, 295, 315, 280],
  ];

  const paletteIndex = key.channel === "RemoteParty" ? 1 : 0;
  const hues = channelPalettes[paletteIndex]!;
  const hue = hues[speakerIndex % hues.length]!;

  return chroma.oklch(0.55, 0.15, hue).hex();
}

export function useSegmentColor(key: SegmentKey): string {
  return useMemo(() => getSegmentColor(key), [key]);
}
