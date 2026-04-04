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

export function StandardTabWrapper({
  children,
  afterBorder,
  floatingButton,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  floatingButton?: React.ReactNode;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="relative flex flex-1 flex-col overflow-hidden rounded-xl border border-neutral-200 bg-white">
        {children}
        {floatingButton}
      </div>
      {afterBorder && <div className="mt-1">{afterBorder}</div>}
    </div>
  );
}
