import { lazy, Suspense, useState } from "react";

import { CalendarNav } from "./calendar";
import { ContactsNav } from "./contacts";
import { ProfileSection } from "./profile";
import { SidebarSearchInput } from "./search";
import { SettingsNav } from "./settings";
import { TemplatesNav } from "./templates";
import { TimelineView } from "./timeline";
import { ToastArea } from "./toast";

import { useShell } from "~/contexts/shell";
import { SearchResults } from "~/search/components/sidebar";
import { useSearch } from "~/search/contexts/ui";
import { useTabs } from "~/store/zustand/tabs";

const DevtoolView = lazy(() =>
  import("./devtool").then((m) => ({ default: m.DevtoolView })),
);

export function LeftSidebar() {
  const { leftsidebar } = useShell();
  const { query } = useSearch();
  const currentTab = useTabs((state) => state.currentTab);
  const [isProfileExpanded, setIsProfileExpanded] = useState(false);

  const isSettingsMode = currentTab?.type === "settings";
  const isCalendarMode = currentTab?.type === "calendar";
  const isContactsMode = currentTab?.type === "contacts";
  const isTemplatesMode = currentTab?.type === "templates";
  const isSpecialMode =
    isSettingsMode || isCalendarMode || isContactsMode || isTemplatesMode;
  const showSearchResults = !isSpecialMode && query.trim() !== "";

  return (
    <div className="flex h-full w-[200px] shrink-0 flex-col gap-1 overflow-hidden pt-1">
      {!isSpecialMode && <SidebarSearchInput />}

      <div className="flex flex-1 flex-col gap-1 overflow-hidden">
        <div className="relative min-h-0 flex-1 overflow-hidden">
          {leftsidebar.showDevtool ? (
            <Suspense fallback={null}>
              <DevtoolView />
            </Suspense>
          ) : isSettingsMode ? (
            <SettingsNav />
          ) : isCalendarMode ? (
            <CalendarNav />
          ) : isContactsMode ? (
            <ContactsNav />
          ) : isTemplatesMode ? (
            <TemplatesNav />
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
          {!leftsidebar.showDevtool && !isSpecialMode && (
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
