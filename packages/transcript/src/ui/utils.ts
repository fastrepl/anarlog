import chroma from "chroma-js";
import { useMemo } from "react";

import type { Segment, SegmentKey, SegmentWord } from "../shared";

export type HighlightSegment = { text: string; isMatch: boolean };

export type AudioState = {
  currentMs: number;
  exists: boolean;
  onSeek: (word: SegmentWord) => void;
};

export type SpeakerInfo = {
  label: string;
  color: string;
};

export type SpeakerLabelResolver = (key: SegmentKey) => SpeakerInfo;

export type Participant = {
  humanId: string;
  name: string;
};

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
        startMs: currentLine[0].start_ms,
        endMs: currentLine[currentLine.length - 1].end_ms,
      });
      currentLine = [];
    }
  }

  if (currentLine.length > 0) {
    lines.push({
      words: currentLine,
      startMs: currentLine[0].start_ms,
      endMs: currentLine[currentLine.length - 1].end_ms,
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

  const firstWord = segment.words[0];
  const lastWord = segment.words[segment.words.length - 1];

  const [from, to] = [firstWord.start_ms, lastWord.end_ms].map(formatTimestamp);
  return `${from} - ${to}`;
}

export function getSegmentColor(key: SegmentKey): string {
  const speakerIndex = key.speaker_index ?? 0;

  const channelPalettes = [
    [10, 25, 0, 340, 15, 350],
    [285, 305, 270, 295, 315, 280],
  ];

  const hues = channelPalettes[key.channel % 2];
  const hue = hues[speakerIndex % hues.length];

  const light = 0.55;
  const chromaVal = 0.15;

  return chroma.oklch(light, chromaVal, hue).hex();
}

export function useSegmentColor(key: SegmentKey): string {
  return useMemo(() => getSegmentColor(key), [key]);
}

export function createSearchHighlightSegments(
  rawText: string,
  query: string,
): HighlightSegment[] {
  const text = rawText.normalize("NFC");
  const lowerText = text.toLowerCase();

  const tokens = query
    .normalize("NFC")
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);
  if (tokens.length === 0) return [{ text, isMatch: false }];

  const ranges: { start: number; end: number }[] = [];
  for (const token of tokens) {
    let cursor = 0;
    let index = lowerText.indexOf(token, cursor);
    while (index !== -1) {
      ranges.push({ start: index, end: index + token.length });
      cursor = index + 1;
      index = lowerText.indexOf(token, cursor);
    }
  }

  if (ranges.length === 0) {
    return [{ text, isMatch: false }];
  }

  ranges.sort((a, b) => a.start - b.start);
  const merged: { start: number; end: number }[] = [{ ...ranges[0] }];
  for (let i = 1; i < ranges.length; i++) {
    const last = merged[merged.length - 1];
    if (ranges[i].start <= last.end) {
      last.end = Math.max(last.end, ranges[i].end);
    } else {
      merged.push({ ...ranges[i] });
    }
  }

  const segments: HighlightSegment[] = [];
  let cursor = 0;
  for (const range of merged) {
    if (range.start > cursor) {
      segments.push({ text: text.slice(cursor, range.start), isMatch: false });
    }
    segments.push({ text: text.slice(range.start, range.end), isMatch: true });
    cursor = range.end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), isMatch: false });
  }

  return segments.length ? segments : [{ text, isMatch: false }];
}
