import { useLayoutEffect, useRef } from "react";

import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { cn } from "@hypr/utils";

const RESIZABLE_AFTER_BORDER_EXPANDED_SIZE = 22;

export { MainShellBodyFrame } from "./body-frame";
export { MainChatPanels } from "./chat-panels";
export { useMainContentCenterOffset } from "./content-offset";
export {
  MainSessionStatusBannerHost,
  SessionStatusBannerProvider,
  useSessionStatusBanner,
} from "./session-status-banner";
export { MainShellScaffold } from "./shell-scaffold";
export { useScrollActiveTabIntoView } from "./tab-scroll";

export function StandardTabWrapper({
  children,
  afterBorder,
  bottomBorderHandle,
  afterBorderResizable = false,
  afterBorderExpanded = false,
  floatingButton,
  mergeAfterBorder = false,
  noBorder = false,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  bottomBorderHandle?: React.ReactNode;
  afterBorderResizable?: boolean;
  afterBorderExpanded?: boolean;
  floatingButton?: React.ReactNode;
  mergeAfterBorder?: boolean;
  noBorder?: boolean;
}) {
  const afterBorderPanelRef = useRef<ImperativePanelHandle>(null);
  const afterBorderSize = RESIZABLE_AFTER_BORDER_EXPANDED_SIZE;
  const hasAfterBorder = Boolean(afterBorder);
  const useResizableAfterBorder =
    hasAfterBorder && afterBorderResizable && afterBorderExpanded;

  useLayoutEffect(() => {
    if (useResizableAfterBorder) {
      afterBorderPanelRef.current?.resize(afterBorderSize);
    }
  }, [afterBorderSize, useResizableAfterBorder]);

  const mainPanel = (
    <MainPanel
      bottomBorderHandle={bottomBorderHandle}
      fill
      floatingButton={floatingButton}
      mergeAfterBorder={mergeAfterBorder}
      noBorder={noBorder}
      afterBorder={afterBorder}
    >
      {children}
    </MainPanel>
  );

  return (
    <div className="flex h-full flex-col">
      <ResizablePanelGroup direction="vertical" className="min-h-0 flex-1">
        <ResizablePanel
          defaultSize={useResizableAfterBorder ? 100 - afterBorderSize : 100}
          minSize={35}
          className="min-h-0"
        >
          {mainPanel}
        </ResizablePanel>
        {useResizableAfterBorder ? (
          <>
            <ResizableHandle
              disabled={!afterBorderExpanded}
              className={cn([
                "z-10 !bg-transparent",
                mergeAfterBorder
                  ? "data-[panel-group-direction=vertical]:h-0"
                  : "data-[panel-group-direction=vertical]:-mb-px",
                !afterBorderExpanded && "pointer-events-none",
              ])}
            />
            <ResizablePanel
              ref={afterBorderPanelRef}
              defaultSize={afterBorderSize}
              minSize={afterBorderExpanded ? 10 : 6}
              maxSize={60}
              className={cn([
                "overflow-hidden",
                mergeAfterBorder ? "min-h-[154px]" : "min-h-[96px]",
              ])}
            >
              <AfterBorderContent
                bottomBorderHandle={bottomBorderHandle}
                fill={afterBorderExpanded}
                mergeAfterBorder={mergeAfterBorder}
              >
                {afterBorder}
              </AfterBorderContent>
            </ResizablePanel>
          </>
        ) : null}
      </ResizablePanelGroup>
      {afterBorder && !useResizableAfterBorder ? (
        <AfterBorderContent
          bottomBorderHandle={bottomBorderHandle}
          mergeAfterBorder={mergeAfterBorder}
        >
          {afterBorder}
        </AfterBorderContent>
      ) : null}
    </div>
  );
}

function MainPanel({
  children,
  afterBorder,
  bottomBorderHandle,
  fill,
  floatingButton,
  mergeAfterBorder,
  noBorder,
}: {
  children: React.ReactNode;
  afterBorder?: React.ReactNode;
  bottomBorderHandle?: React.ReactNode;
  fill: boolean;
  floatingButton?: React.ReactNode;
  mergeAfterBorder: boolean;
  noBorder: boolean;
}) {
  return (
    <div
      className={cn([
        "relative flex min-h-0 flex-1 flex-col",
        fill && "h-full",
      ])}
    >
      <div
        data-chat-floating-anchor
        data-main-has-after-border={afterBorder ? "" : undefined}
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
  );
}

function AfterBorderContent({
  children,
  bottomBorderHandle,
  fill = false,
  mergeAfterBorder,
}: {
  children: React.ReactNode;
  bottomBorderHandle?: React.ReactNode;
  fill?: boolean;
  mergeAfterBorder: boolean;
}) {
  return (
    <div
      className={cn([
        !mergeAfterBorder && (bottomBorderHandle ? "pt-1.5" : "mt-1"),
        fill && "flex h-full min-h-0 flex-col overflow-hidden",
      ])}
    >
      {children}
    </div>
  );
}
