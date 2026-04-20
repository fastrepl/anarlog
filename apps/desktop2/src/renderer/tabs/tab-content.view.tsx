import { WrenchIcon } from "lucide-react";

import { SessionEditorContainer } from "~/sessions";
import { SettingsContainer } from "~/settings";
import { NotPortedYetView } from "~/tabs/not-ported-yet.view";
import { getDailySummaryLabel, type Tab } from "~/tabs/tabs.types";

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
    return <DailySummaryPlaceholderView date={tab.date} />;
  }

  if (tab.type === "settings") {
    return <SettingsContainer />;
  }

  return <NotPortedYetView kind={tab.type} />;
}

function DailySummaryPlaceholderView({ date }: { date: string }) {
  return (
    <div className="grid h-full place-content-center px-6">
      <div className="flex max-w-sm flex-col items-center gap-3 text-center">
        <div className="flex size-10 items-center justify-center rounded-full border border-neutral-200 bg-neutral-50 text-neutral-500">
          <WrenchIcon size={18} />
        </div>
        <div className="text-lg font-semibold text-neutral-900">
          {getDailySummaryLabel(date)}
        </div>
        <p className="text-sm text-neutral-500">
          This surface is not ported to `desktop2` yet.
        </p>
      </div>
    </div>
  );
}
