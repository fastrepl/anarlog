import { Fragment } from "react";

import { cn } from "@hypr/utils";

import { SyncProvider } from "~/calendar/components/context";
import { useTabs } from "~/store/zustand/tabs";

export function MainShellScaffold({
  children,
  edgeToEdge = false,
}: {
  children: React.ReactNode;
  edgeToEdge?: boolean;
}) {
  const currentTab = useTabs((state) => state.currentTab);
  const isCalendarMode = currentTab?.type === "calendar";
  const SyncWrapper = isCalendarMode ? SyncProvider : Fragment;

  return (
    <SyncWrapper>
      <div
        className={cn([
          "flex h-full gap-1 overflow-hidden bg-stone-50",
          !edgeToEdge && "px-1 pb-1",
          edgeToEdge && [
            "[&_[data-chat-floating-anchor]]:rounded-none",
            "[&_[data-chat-floating-anchor]]:border-x-0",
            "[&_[data-chat-floating-anchor]]:border-t",
            "[&_[data-chat-floating-anchor]]:border-b-0",
          ],
        ])}
        data-testid="main-app-shell"
      >
        {children}
      </div>
    </SyncWrapper>
  );
}
