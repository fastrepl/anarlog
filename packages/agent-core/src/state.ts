import { Annotation, MessagesAnnotation } from "@langchain/langgraph";

import type { ImageContent } from "./utils/input";

export const AgentState = Annotation.Root({
  ...MessagesAnnotation.spec,

  request: Annotation<string>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => "",
  }),

  images: Annotation<ImageContent[]>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => [],
  }),

  output: Annotation<string>({
    reducer: (prev, newValue) => newValue ?? prev,
    default: () => "",
  }),
});

export type AgentStateType = typeof AgentState.State;
