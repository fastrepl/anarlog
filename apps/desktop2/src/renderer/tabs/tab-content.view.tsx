import { SessionEditorContainer } from "~/home";
import { NotPortedYetView } from "~/tabs/not-ported-yet.view";
import type { Tab } from "~/tabs/tabs.types";

export function TabContentView({
  tab,
  homeContent,
}: {
  tab: Tab | null;
  homeContent?: React.ReactNode;
}) {
  if (!tab) {
    return <>{homeContent ?? null}</>;
  }

  if (tab.type === "sessions") {
    return <SessionEditorContainer sessionId={tab.id} />;
  }

  if (tab.type === "daily-summary") {
    return <NotPortedYetView kind={tab.type} hint={tab.date} />;
  }

  return <NotPortedYetView kind={tab.type} />;
}
