import { createFileRoute } from "@tanstack/react-router";
import { useLayoutEffect, useMemo } from "react";

import { ClassicMainLayout } from "~/main/layout";
import { TabContentNote } from "~/session";
import { StandaloneWindowShell } from "~/shared/window-shell";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const Route = createFileRoute("/app/note/$sessionId")({
  component: Component,
});

function Component() {
  const { sessionId } = Route.useParams();
  const tab = useStandaloneNoteTab(sessionId);

  return (
    <ClassicMainLayout includeServices={false}>
      <StandaloneWindowShell topDragRegion={false}>
        <div className="bg-background h-screen w-screen">
          <TabContentNote tab={tab} standaloneWindow />
        </div>
      </StandaloneWindowShell>
    </ClassicMainLayout>
  );
}

export function useStandaloneNoteTab(sessionId: string) {
  const tab = useMemo(
    () =>
      ({
        active: true,
        id: sessionId,
        pinned: false,
        slotId: `note-window-${sessionId}`,
        state: { view: null, autoStart: null },
        type: "sessions",
      }) satisfies Extract<Tab, { type: "sessions" }>,
    [sessionId],
  );

  const storeTab = useTabs((state) =>
    state.tabs.find(
      (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
        candidate.type === "sessions" && candidate.id === sessionId,
    ),
  );

  useLayoutEffect(() => {
    const state = useTabs.getState();
    const existingTab = state.tabs.find(
      (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
        candidate.type === "sessions" && candidate.id === sessionId,
    );

    if (existingTab) {
      state.select(existingTab);
      return;
    }

    state.openCurrent(tab);
  }, [sessionId, tab]);

  return storeTab ?? tab;
}
