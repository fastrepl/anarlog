import type { StoreApi } from "zustand";

export type ChatMode = "RightPanelOpen" | "FloatingClosed";

export type ChatEvent =
  | { type: "OPEN" }
  | { type: "OPEN_RIGHT_PANEL" }
  | { type: "CLOSE" }
  | { type: "TOGGLE" };

export type ChatModeState = {
  chatMode: ChatMode;
};

export type ChatModeActions = {
  transitionChatMode: (event: ChatEvent) => void;
};

const computeNextChatMode = (state: ChatMode, event: ChatEvent): ChatMode => {
  switch (state) {
    case "RightPanelOpen":
      if (event.type === "CLOSE" || event.type === "TOGGLE") {
        return "FloatingClosed";
      }
      return state;
    case "FloatingClosed":
      if (
        event.type === "OPEN" ||
        event.type === "OPEN_RIGHT_PANEL" ||
        event.type === "TOGGLE"
      ) {
        return "RightPanelOpen";
      }
      return state;
    default:
      return state;
  }
};

export const createChatModeSlice = <T extends ChatModeState>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): ChatModeState & ChatModeActions => ({
  chatMode: "FloatingClosed",
  transitionChatMode: (event) => {
    const currentMode = get().chatMode;
    const nextMode = computeNextChatMode(currentMode, event);
    if (nextMode === currentMode) return;

    set({
      chatMode: nextMode,
    } as Partial<T>);
  },
});
