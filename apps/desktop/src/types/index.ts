import type { ToolRegistry } from "~/contexts/tool-registry/core";
import type { AITaskStore } from "~/store/zustand/ai-task";
import type { ListenerStore } from "~/store/zustand/listener";

export type Context = {
  listenerStore: ListenerStore;
  aiTaskStore: AITaskStore;
  toolRegistry: ToolRegistry;
};
