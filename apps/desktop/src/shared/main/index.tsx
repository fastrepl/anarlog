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
export { useChatPanelToolbarWidth } from "./chat-panel-toolbar-width";
export { useScrollActiveTabIntoView } from "./tab-scroll";

export function StandardTabWrapper({
  children,
  afterBorder,
  bottomBorderHandle,
  floatingButton,
  mergeAfterBorder = false,
  noBorder = false,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  bottomBorderHandle?: React.ReactNode;
  floatingButton?: React.ReactNode;
  mergeAfterBorder?: boolean;
  noBorder?: boolean;
}) {
  return (
    <div className="flex h-full flex-col">
      <div className="relative flex min-h-0 flex-1 flex-col">
        <div
          data-chat-floating-anchor
          className={cn([
            "relative flex min-h-0 flex-1 flex-col overflow-hidden bg-white",
            mergeAfterBorder && afterBorder
              ? "rounded-t-xl rounded-b-none"
              : "rounded-xl",
            !noBorder &&
              (mergeAfterBorder && afterBorder
                ? "border border-b-0 border-neutral-200"
                : "border border-neutral-200"),
          ])}
        >
          {children}
          {floatingButton}
        </div>
        {bottomBorderHandle ? (
          <div className="pointer-events-none absolute right-0 bottom-0 left-0 z-20">
            <div className="pointer-events-auto relative">
              {bottomBorderHandle}
            </div>
          </div>
        ) : null}
      </div>
      {afterBorder ? (
        <div
          className={cn([
            !mergeAfterBorder && (bottomBorderHandle ? "pt-[10px]" : "mt-1"),
          ])}
        >
          {afterBorder}
        </div>
      ) : null}
    </div>
  );
}
