import type { SpeakerHintStorage, WordStorage } from "@anlg/store";

export type WordWithId = WordStorage & { id: string };
export type SpeakerHintWithId = SpeakerHintStorage & { id: string };
