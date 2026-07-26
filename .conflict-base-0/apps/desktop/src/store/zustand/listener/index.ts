import { createStore } from "zustand";

import { type BatchActions, type BatchState, createBatchSlice } from "./batch";
import {
  createGeneralSlice,
  type GeneralActions,
  type GeneralState,
  type SessionMode,
} from "./general";
import {
  createTranscriptSlice,
  type TranscriptActions,
  type TranscriptState,
} from "./transcript";

type State = GeneralState & TranscriptState & BatchState;
type Actions = GeneralActions & TranscriptActions & BatchActions;
type Store = State & Actions;

export type ListenerStore = ReturnType<typeof createListenerStore>;
export type { SessionMode };

export const createListenerStore = () => {
  return createStore<Store>((set, get) => ({
    ...createGeneralSlice(set, get),
    ...createTranscriptSlice(set, get),
    ...createBatchSlice(set, get),
  }));
};
