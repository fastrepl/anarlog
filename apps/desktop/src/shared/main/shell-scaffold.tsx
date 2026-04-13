import { Fragment } from "react";

import { SyncProvider } from "~/calendar/components/context";
import { useTabs } from "~/store/zustand/tabs";

export function MainShellScaffold({ children }: { children: React.ReactNode }) {
  const currentTab = useTabs((state) => state.currentTab);
  const isCalendarMode = currentTab?.type === "calendar";
  const SyncWrapper = isCalendarMode ? SyncProvider : Fragment;

  return (
    <SyncWrapper>
      <div
        className="flex h-full gap-1 overflow-hidden bg-stone-50 px-1 pb-1"
        data-testid="main-app-shell"
      >
        {children}
      </div>
    </SyncWrapper>
  );
}
