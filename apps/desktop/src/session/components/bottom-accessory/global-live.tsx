import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

import {
  type ImperativePanelHandle,
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@hypr/ui/components/ui/resizable";
import { cn } from "@hypr/utils";

import { DuringSessionAccessory } from "./during-session";
import { ExpandToggle } from "./expand-toggle";
import { shouldShowLiveTranscriptAccessory } from "./live-visibility";

import { type MainSurfaceChrome } from "~/shared/main";
import { type Tab } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

const GLOBAL_LIVE_AFTER_BORDER_EXPANDED_SIZE = 22;

export function GlobalLiveTranscriptAccessory({
  children,
  currentTab,
  surfaceChrome = "default",
}: {
  children: ReactNode;
  currentTab: Tab | null;
  surfaceChrome?: MainSurfaceChrome;
}) {
  const live = useListener((state) => ({
    status: state.live.status,
    sessionId: state.live.sessionId,
    requestedLiveTranscription: state.live.requestedLiveTranscription,
    liveTranscriptionActive: state.live.liveTranscriptionActive,
  }));
  const sessionId = live.sessionId;
  const currentTabOwnsLiveAccessory =
    currentTab?.type === "sessions" && currentTab.id === sessionId;
  const accessorySessionId =
    sessionId &&
    !currentTabOwnsLiveAccessory &&
    shouldShowLiveTranscriptAccessory(live)
      ? sessionId
      : null;

  return (
    <GlobalLiveTranscriptAccessoryContent
      sessionId={accessorySessionId}
      surfaceChrome={surfaceChrome}
    >
      {children}
    </GlobalLiveTranscriptAccessoryContent>
  );
}

function GlobalLiveTranscriptAccessoryContent({
  children,
  sessionId,
  surfaceChrome,
}: {
  children: ReactNode;
  sessionId: string | null;
  surfaceChrome: MainSurfaceChrome;
}) {
  const [expandedLiveSession, setExpandedLiveSession] = useState<{
    sessionId: string | null;
    isExpanded: boolean;
  }>({ sessionId: null, isExpanded: false });
  const afterBorderPanelRef = useRef<ImperativePanelHandle>(null);
  const afterBorderSize = GLOBAL_LIVE_AFTER_BORDER_EXPANDED_SIZE;
  const hasLiveAccessory = Boolean(sessionId);
  const canExpand = hasLiveAccessory;
  const isExpanded =
    canExpand &&
    expandedLiveSession.sessionId === sessionId &&
    expandedLiveSession.isExpanded;
  const useResizableAfterBorder = canExpand && isExpanded;

  useLayoutEffect(() => {
    if (useResizableAfterBorder) {
      afterBorderPanelRef.current?.resize(afterBorderSize);
    }
  }, [afterBorderSize, useResizableAfterBorder]);

  const bottomBorderHandle = canExpand ? (
    <ExpandToggle
      isExpanded={isExpanded}
      onToggle={() =>
        setExpandedLiveSession((value) => ({
          sessionId,
          isExpanded: value.sessionId === sessionId ? !value.isExpanded : true,
        }))
      }
      label="Live"
      collapsedClassName="bg-muted"
      expandedClassName="bg-muted"
    />
  ) : null;

  const afterBorder = sessionId ? (
    <DuringSessionAccessory
      sessionId={sessionId}
      isExpanded={isExpanded}
      fillHeight={useResizableAfterBorder}
    />
  ) : null;

  return (
    <div
      data-global-live-transcript-shell
      className={cn([
        "flex h-full flex-col",
        hasLiveAccessory && "[&_[data-chat-floating-anchor]]:!border-b",
        hasLiveAccessory &&
          surfaceChrome === "left" &&
          "[&_[data-chat-floating-anchor]]:!rounded-bl-none",
      ])}
    >
      <ResizablePanelGroup direction="vertical" className="min-h-0 flex-1">
        <ResizablePanel
          defaultSize={useResizableAfterBorder ? 100 - afterBorderSize : 100}
          minSize={35}
          className="min-h-0"
        >
          <div className="relative h-full min-h-0">
            {children}
            {bottomBorderHandle ? (
              <div className="pointer-events-none absolute right-0 bottom-0 left-0 z-20">
                <div className="pointer-events-auto relative">
                  {bottomBorderHandle}
                </div>
              </div>
            ) : null}
          </div>
        </ResizablePanel>
        {useResizableAfterBorder ? (
          <>
            <ResizableHandle className="z-10 !bg-transparent data-[panel-group-direction=vertical]:-mb-px" />
            <ResizablePanel
              ref={afterBorderPanelRef}
              defaultSize={afterBorderSize}
              minSize={10}
              maxSize={60}
              className="min-h-[96px] overflow-hidden"
            >
              <GlobalLiveAfterBorderContent
                bottomBorderHandle={bottomBorderHandle}
                fill
              >
                {afterBorder}
              </GlobalLiveAfterBorderContent>
            </ResizablePanel>
          </>
        ) : null}
      </ResizablePanelGroup>
      {afterBorder && !useResizableAfterBorder ? (
        <GlobalLiveAfterBorderContent bottomBorderHandle={bottomBorderHandle}>
          {afterBorder}
        </GlobalLiveAfterBorderContent>
      ) : null}
    </div>
  );
}

function GlobalLiveAfterBorderContent({
  children,
  bottomBorderHandle,
  fill = false,
}: {
  children: ReactNode;
  bottomBorderHandle: ReactNode;
  fill?: boolean;
}) {
  return (
    <div
      data-global-live-transcript-card
      className={cn([
        !bottomBorderHandle && "mt-1",
        fill && "flex h-full min-h-0 flex-col overflow-hidden",
      ])}
    >
      {children}
    </div>
  );
}
