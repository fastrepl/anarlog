import { createFileRoute } from "@tanstack/react-router";
import { FileText, Pin, Settings } from "lucide-react";

import {
  LiveSession,
  Notepad,
  PrefilledDemo,
  SettingsPanel,
} from "@/components/notepad";

export const Route = createFileRoute("/_view/app/file-transcription")({
  component: Component,
  validateSearch: (search: Record<string, unknown>) => ({
    id: (search.id as string) || undefined,
    tab: (search.tab as string) || undefined,
  }),
});

function Component() {
  const { id: searchId, tab } = Route.useSearch();

  return (
    <Notepad
      searchId={searchId}
      activeTab={tab}
      tabs={[
        {
          value: "default",
          icon: FileText,
          sessionId: "default",
          content: <LiveSession />,
        },
        {
          value: "demo",
          icon: Pin,
          pinned: true,
          sessionId: "demo",
          title: "Content Strategy & Publishing Cadence",
          content: <PrefilledDemo sessionId="demo" />,
        },
        {
          value: "settings",
          icon: Settings,
          label: "Settings",
          content: <SettingsPanel />,
        },
      ]}
    />
  );
}
