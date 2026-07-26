import { useEffect, useState } from "react";

export type TranscriptLine = {
  id: string;
  speaker: string;
  text: string;
};

const script: Array<[string, string]> = [
  ["Sam", "Okay, I think everyone's here — let's get started."],
  ["You", "Sounds good. Quick recap of last week first?"],
  [
    "Sam",
    "Sure. We shipped the sync fixes and the crash rate is back to baseline.",
  ],
  ["You", "Nice. Anything left over from the incident review?"],
  [
    "Sam",
    "Two follow-ups: retry backoff on the relay, and better offline messaging.",
  ],
  ["You", "I can take the offline messaging one this week."],
  [
    "Sam",
    "Perfect. Next up is the mobile scaffold — designs are in the project.",
  ],
  [
    "You",
    "I saw them. Timeline home screen plus a note view with live transcript.",
  ],
  ["Sam", "Right, and capture should stay local-first like desktop."],
  ["You", "Agreed. Let's scope v1 after the sync foundation lands."],
];

// Simulates a streaming STT source until the mobile-bridge transport exists.
export function useMockLiveTranscript(): TranscriptLine[] {
  const [lines, setLines] = useState<TranscriptLine[]>([]);

  useEffect(() => {
    const interval = setInterval(() => {
      setLines((current) => {
        if (current.length >= script.length) return current;
        const [speaker, text] = script[current.length]!;
        return [...current, { id: `line-${current.length}`, speaker, text }];
      });
    }, 1600);
    return () => clearInterval(interval);
  }, []);

  return lines;
}
