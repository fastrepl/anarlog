import type { StoreApi } from "zustand";

import type { BasicActions, BasicState } from "./basic";

export type ChatMode =
  | "RightPanelOpen"
  | "FloatingClosed"
  | "FloatingOpen"
  | "FullTab";

export type ChatEvent =
  | { type: "OPEN" }
  | { type: "CLOSE" }
  | { type: "SHIFT" }
  | { type: "TOGGLE" }
  | { type: "OPEN_TAB" };

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
      if (event.type === "SHIFT") {
        return "FloatingOpen";
      }
      if (event.type === "OPEN_TAB") {
        return "FullTab";
      }
      return state;
    case "FloatingClosed":
      if (event.type === "OPEN" || event.type === "TOGGLE") {
        return "FloatingOpen";
      }
      if (event.type === "OPEN_TAB") {
        return "FullTab";
      }
      return state;
    case "FloatingOpen":
      if (event.type === "CLOSE" || event.type === "TOGGLE") {
        return "FloatingClosed";
      }
      if (event.type === "SHIFT") {
        return "RightPanelOpen";
      }
      if (event.type === "OPEN_TAB") {
        return "FullTab";
      }
      return state;
    case "FullTab":
      if (event.type === "CLOSE" || event.type === "TOGGLE") {
        return "FloatingClosed";
      }
      return state;
    default:
      return state;
  }
};

export const createChatModeSlice = <
  T extends ChatModeState & BasicState & BasicActions,
>(
  set: StoreApi<T>["setState"],
  get: StoreApi<T>["getState"],
): ChatModeState & ChatModeActions => ({
  chatMode: "FloatingClosed",
  transitionChatMode: (event) => {
    const currentMode = get().chatMode;
    const nextMode = computeNextChatMode(currentMode, event);
    if (nextMode === currentMode) return;

    set({ chatMode: nextMode } as Partial<T>);

    if (currentMode === "FullTab" && nextMode !== "FullTab") {
      const chatTab = get().tabs.find((t) => t.type === "chat_support");
      if (chatTab) {
        get().close(chatTab);
      }
    }
  },
});
