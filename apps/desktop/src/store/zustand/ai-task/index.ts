import { createStore } from "zustand";

import { createTasksSlice, type TasksActions, type TasksState } from "./tasks";

import type { Store as MainStore } from "~/store/tinybase/store/main";

type State = TasksState;
type Actions = TasksActions;
type Store = State & Actions;

export type AITaskStore = ReturnType<typeof createAITaskStore>;

export const createAITaskStore = ({
  persistedStore,
}: {
  persistedStore: MainStore;
}) => {
  return createStore<Store>((set, get) => ({
    ...createTasksSlice(set, get, { persistedStore }),
  }));
};
