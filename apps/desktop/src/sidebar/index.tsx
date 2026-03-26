import { useQuery } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { AxeIcon, PanelLeftCloseIcon } from "lucide-react";
import { lazy, Suspense, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Kbd } from "@hypr/ui/components/ui/kbd";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { ProfileSection } from "./profile";
import { SidebarSearchInput } from "./search";
import { SettingsNav } from "./settings-nav";
import { TimelineView } from "./timeline";
import { ToastArea } from "./toast";

import { useShell } from "~/contexts/shell";
import { SearchResults } from "~/search/components/sidebar";
import { useSearch } from "~/search/contexts/ui";
import { TrafficLights } from "~/shared/ui/traffic-lights";
import { useTabs } from "~/store/zustand/tabs";
import { commands } from "~/types/tauri.gen";

const DevtoolView = lazy(() =>
  import("./devtool").then((m) => ({ default: m.DevtoolView })),
);

export function LeftSidebar() {
  const { leftsidebar } = useShell();
  const { query } = useSearch();
  const currentTab = useTabs((state) => state.currentTab);
  const [isProfileExpanded, setIsProfileExpanded] = useState(false);
  const isLinux = platform() === "linux";

  const { data: showDevtoolButton = false } = useQuery({
    queryKey: ["show_devtool"],
    queryFn: () => commands.showDevtool(),
  });

  const isSettingsMode = currentTab?.type === "settings";
  const showSearchResults = !isSettingsMode && query.trim() !== "";

  return (
    <div className="flex h-full w-70 shrink-0 flex-col gap-1 overflow-hidden">
      <header
        data-tauri-drag-region
        className={cn([
          "flex flex-row items-center",
          "h-9 w-full py-1",
          isLinux ? "justify-between pl-3" : "justify-end pl-20",
          "shrink-0",
        ])}
      >
        {isLinux && <TrafficLights />}
        <div className="flex items-center">
          {showDevtoolButton && (
            <Button
              size="icon"
              variant="ghost"
              onClick={leftsidebar.toggleDevtool}
            >
              <AxeIcon size={16} />
            </Button>
          )}
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                size="icon"
                variant="ghost"
                onClick={leftsidebar.toggleExpanded}
              >
                <PanelLeftCloseIcon size={16} />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="flex items-center gap-2">
              <span>Toggle sidebar</span>
              <Kbd className="animate-kbd-press">⌘ \</Kbd>
            </TooltipContent>
          </Tooltip>
        </div>
      </header>

      {!isSettingsMode && <SidebarSearchInput />}

      <div className="flex flex-1 flex-col gap-1 overflow-hidden">
        <div className="relative min-h-0 flex-1 overflow-hidden">
          {leftsidebar.showDevtool ? (
            <Suspense fallback={null}>
              <DevtoolView />
            </Suspense>
          ) : isSettingsMode ? (
            <SettingsNav />
          ) : (
            <>
              <div className={showSearchResults ? "h-full" : "hidden"}>
                <SearchResults />
              </div>
              <div className={showSearchResults ? "hidden" : "h-full"}>
                <TimelineView />
              </div>
            </>
          )}
          {!leftsidebar.showDevtool && !isSettingsMode && (
            <ToastArea isProfileExpanded={isProfileExpanded} />
          )}
        </div>
        <div className="relative z-30">
          <ProfileSection onExpandChange={setIsProfileExpanded} />
        </div>
      </div>
    </div>
  );
}
