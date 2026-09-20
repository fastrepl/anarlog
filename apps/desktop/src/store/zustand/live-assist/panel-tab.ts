import { create } from "zustand";

// Which content the right panel's Chat/Live Assist switcher shows. Kept
// separate from the chat right-panel open/closed state (`useShell().chat`,
// backed by `store/zustand/tabs/chat-mode.ts`): that state answers whether
// the panel is open, this one answers which tab is selected inside it.
export type LiveAssistPanelTab = "chat" | "live_assist";

type LiveAssistPanelTabState = {
  activeTab: LiveAssistPanelTab;
  setActiveTab: (tab: LiveAssistPanelTab) => void;
};

export const useLiveAssistPanelTab = create<LiveAssistPanelTabState>((set) => ({
  activeTab: "chat",
  setActiveTab: (tab) => set({ activeTab: tab }),
}));
