import { createFileRoute } from "@tanstack/react-router";

import { ClassicMainLayout } from "~/main/layout";
import { FloatingMeetingPanel } from "~/meeting-float/panel";

export const Route = createFileRoute("/floating")({
  validateSearch: (
    search,
  ): {
    sessionId?: string;
    showTranscriptTab: boolean;
  } => {
    const params = search as {
      sessionId?: unknown;
      showTranscriptTab?: unknown;
    };

    return {
      sessionId:
        typeof params.sessionId === "string" ? params.sessionId : undefined,
      showTranscriptTab: parseBooleanSearchParam(
        params.showTranscriptTab,
        true,
      ),
    };
  },
  component: Component,
});

function Component() {
  const { sessionId, showTranscriptTab } = Route.useSearch();

  if (!sessionId) {
    return <div className="h-screen w-screen bg-transparent" />;
  }

  return (
    <ClassicMainLayout includeServices={false}>
      <FloatingMeetingPanel
        key={sessionId}
        sessionId={sessionId}
        initialShowTranscriptTab={showTranscriptTab}
      />
    </ClassicMainLayout>
  );
}

function parseBooleanSearchParam(value: unknown, fallback: boolean) {
  if (value === true || value === "1" || value === "true") {
    return true;
  }

  if (value === false || value === "0" || value === "false") {
    return false;
  }

  return fallback;
}
