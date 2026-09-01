import { platform } from "@tauri-apps/plugin-os";
import { Fragment } from "react";

import { cn } from "@anlg/utils";

import { SyncProvider } from "~/calendar/components/context";
import { useTabs } from "~/store/zustand/tabs";

export type MainSurfaceChrome = "default" | "top" | "top-borderless" | "left";

export function MainShellScaffold({
  children,
  edgeToEdge = false,
  mainSurfaceChrome,
}: {
  children: React.ReactNode;
  edgeToEdge?: boolean;
  mainSurfaceChrome?: MainSurfaceChrome;
}) {
  const currentTab = useTabs((state) => state.currentTab);
  const isCalendarMode = currentTab?.type === "calendar";
  const isMacos = platform() === "macos";
  const SyncWrapper = isCalendarMode ? SyncProvider : Fragment;
  const resolvedMainSurfaceChrome =
    mainSurfaceChrome ?? (edgeToEdge ? "top" : "default");
  const hasTopMainSurfaceChrome =
    resolvedMainSurfaceChrome === "top" ||
    resolvedMainSurfaceChrome === "top-borderless";

  return (
    <SyncWrapper>
      <div
        className={cn([
          "bg-background flex h-full gap-1 overflow-hidden",
          !hasTopMainSurfaceChrome && "pl-1",
          hasTopMainSurfaceChrome && [
            isMacos && "[&_[data-chat-floating-anchor]]:rounded-t-xl",
            "[&_[data-chat-floating-anchor]]:rounded-b-none",
            "[&_[data-chat-floating-anchor]]:border-x-0",
            resolvedMainSurfaceChrome === "top"
              ? "[&_[data-chat-floating-anchor]]:border-t"
              : "[&_[data-chat-floating-anchor]]:!border-t-0",
            "[&_[data-chat-floating-anchor]]:border-b-0",
          ],
          resolvedMainSurfaceChrome === "left" && [
            isMacos && "[&_[data-chat-floating-anchor]]:rounded-l-xl",
            !isMacos && "[&_[data-chat-floating-anchor]]:rounded-tl-xl",
            "[&_[data-chat-floating-anchor]]:rounded-r-none",
            "[&_[data-chat-floating-anchor]]:border-y-0",
            "[&_[data-chat-floating-anchor]]:border-r-0",
            "[&_[data-chat-floating-anchor]]:border-l",
          ],
          hasTopMainSurfaceChrome &&
            !isMacos &&
            "[&_[data-chat-floating-anchor]]:rounded-t-xl",
        ])}
        data-testid="main-app-shell"
      >
        {children}
      </div>
    </SyncWrapper>
  );
}
