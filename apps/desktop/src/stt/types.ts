import type { SpeakerHintStorage, WordStorage } from "@hypr/store";

export type WordWithId = WordStorage & { id: string };
export type SpeakerHintWithId = SpeakerHintStorage & { id: string };
