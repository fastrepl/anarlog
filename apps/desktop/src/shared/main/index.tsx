import { cn } from "@hypr/utils";

export { MainShellBodyFrame } from "./body-frame";
export { MainChatPanels } from "./chat-panels";
export { useMainContentCenterOffset } from "./content-offset";
export {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";
export { MainShellScaffold } from "./shell-scaffold";
export { MainTabItem } from "./tab-item";
export { MainTabContent } from "./tab-content";
export { useScrollActiveTabIntoView } from "./tab-scroll";

export function StandardTabWrapper({
  children,
  afterBorder,
  floatingButton,
  noBorder = false,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  floatingButton?: React.ReactNode;
  noBorder?: boolean;
}) {
  return (
    <div className="flex h-full flex-col">
      <div
        className={cn([
          "relative flex flex-1 flex-col overflow-hidden rounded-xl bg-white",
          !noBorder && "border border-neutral-200",
        ])}
      >
        {children}
        {floatingButton}
      </div>
      {afterBorder && <div className="mt-1">{afterBorder}</div>}
    </div>
  );
}
