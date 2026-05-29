import { Fragment } from "react";

import { cn } from "@hypr/utils";

import { SyncProvider } from "~/calendar/components/context";
import { useTabs } from "~/store/zustand/tabs";

export function MainShellScaffold({
  children,
  edgeToEdge = false,
  mainSurfaceChrome,
}: {
  children: React.ReactNode;
  edgeToEdge?: boolean;
  mainSurfaceChrome?: "default" | "top" | "left";
}) {
  const currentTab = useTabs((state) => state.currentTab);
  const isCalendarMode = currentTab?.type === "calendar";
  const SyncWrapper = isCalendarMode ? SyncProvider : Fragment;
  const resolvedMainSurfaceChrome =
    mainSurfaceChrome ?? (edgeToEdge ? "top" : "default");

  return (
    <SyncWrapper>
      <div
        className={cn([
          "flex h-full gap-1 overflow-hidden bg-stone-50",
          resolvedMainSurfaceChrome !== "top" && "pl-1",
          resolvedMainSurfaceChrome === "top" && [
            "[&_[data-chat-floating-anchor]]:rounded-t-xl",
            "[&_[data-chat-floating-anchor]]:rounded-b-none",
            "[&_[data-chat-floating-anchor]]:border-x-0",
            "[&_[data-chat-floating-anchor]]:border-t",
            "[&_[data-chat-floating-anchor]]:border-b-0",
          ],
          resolvedMainSurfaceChrome === "left" && [
            "[&_[data-chat-floating-anchor]]:rounded-l-xl",
            "[&_[data-chat-floating-anchor]]:rounded-r-none",
            "[&_[data-chat-floating-anchor][data-main-has-after-border]]:rounded-bl-none",
            "[&_[data-chat-floating-anchor]]:border-y-0",
            "[&_[data-chat-floating-anchor]]:border-r-0",
            "[&_[data-chat-floating-anchor]]:border-l",
          ],
        ])}
        data-testid="main-app-shell"
      >
        {children}
      </div>
    </SyncWrapper>
  );
}
