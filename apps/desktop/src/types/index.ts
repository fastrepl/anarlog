import type { ToolRegistry } from "~/contexts/tool-registry/core";
import { type Store as InternalStore } from "~/store/tinybase/store/main";
import { type Store as MainStore } from "~/store/tinybase/store/main";
import type { AITaskStore } from "~/store/zustand/ai-task";
import type { ListenerStore } from "~/store/zustand/listener";

export type Context = {
  persistedStore: MainStore;
  internalStore: InternalStore;
  listenerStore: ListenerStore;
  aiTaskStore: AITaskStore;
  toolRegistry: ToolRegistry;
};
