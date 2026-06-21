import { createFileRoute } from "@tanstack/react-router";
import { useMemo } from "react";

import { ClassicMainLayout } from "~/main/layout";
import { TabContentNote } from "~/session";
import { StandaloneWindowShell } from "~/shared/window-shell";
import type { Tab } from "~/store/zustand/tabs";

export const Route = createFileRoute("/app/note/$sessionId")({
  component: Component,
});

function Component() {
  const { sessionId } = Route.useParams();
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

  return (
    <ClassicMainLayout includeServices={false}>
      <StandaloneWindowShell>
        <div className="bg-background h-screen w-screen">
          <TabContentNote tab={tab} standaloneWindow />
        </div>
      </StandaloneWindowShell>
    </ClassicMainLayout>
  );
}
