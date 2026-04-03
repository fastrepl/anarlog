import { createContext, useContext } from "react";

import { StandardTabChatButton } from "./tab-chrome";

export { MainShellBodyFrame } from "./body-frame";
export { Body } from "./body";
export { MainChatPanels } from "./chat-panels";
export { useMainContentCenterOffset } from "./content-offset";
export {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";
export { MainShellScaffold } from "./shell-scaffold";
export { MainShellSidebar } from "./shell-sidebar";
export { MainShellFrame } from "./shell-frame";
export { MainTabChrome, MainTabItem, useMainTabsShortcuts } from "./tab-chrome";
export { MainTabContent } from "./tab-content";
export { useScrollActiveTabIntoView } from "./tab-scroll";

const MainChromeContext = createContext({
  showFloatingChatButton: true,
});

export function MainChromeProvider({
  children,
  showFloatingChatButton = true,
}: {
  children: React.ReactNode;
  showFloatingChatButton?: boolean;
}) {
  return (
    <MainChromeContext.Provider value={{ showFloatingChatButton }}>
      {children}
    </MainChromeContext.Provider>
  );
}
export function StandardTabWrapper({
  children,
  afterBorder,
  floatingButton,
  showTimeline = false,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  floatingButton?: React.ReactNode;
  showTimeline?: boolean;
}) {
  const { showFloatingChatButton } = useContext(MainChromeContext);

  return (
    <div className="flex h-full flex-col">
      <div className="relative flex flex-1 flex-col overflow-hidden rounded-xl border border-neutral-200 bg-white">
        {children}
        {floatingButton}
        {showFloatingChatButton && (
          <StandardTabChatButton showTimeline={showTimeline} />
        )}
      </div>
      {afterBorder && <div className="mt-1">{afterBorder}</div>}
    </div>
  );
}
