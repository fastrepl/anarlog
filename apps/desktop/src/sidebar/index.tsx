import { type ReactNode } from "react";

import { cn } from "@anlg/utils";

import { AutomationsNav } from "./automations";
import { CalendarNav } from "./calendar";
import { ContactsNav } from "./contacts";
import { FolderMaterialsPanel } from "./folder-materials";
import type { SidebarNoteFilter } from "./note-filter";
import { SettingsNav } from "./settings";
import { SharedNotesNav } from "./shared-notes";
import { TemplatesNav } from "./templates";
import { TimelineView } from "./timeline";
import { hasOwnSidebarHeaderTab } from "./use-custom-sidebar";

import { useTabs } from "~/store/zustand/tabs";

export function LeftSidebar({
  folderFilter = null,
  noteFilter = "mine",
  timelineHeader,
  showIgnoredTimelineEvents,
  onShowIgnoredTimelineEventsChange,
}: {
  folderFilter?: string | null;
  noteFilter?: SidebarNoteFilter;
  timelineHeader?: ReactNode;
  showIgnoredTimelineEvents?: boolean;
  onShowIgnoredTimelineEventsChange?: (showIgnored: boolean) => void;
} = {}) {
  const currentTab = useTabs((state) => state.currentTab);

  const isSettingsMode = currentTab?.type === "settings";
  const isCalendarMode = currentTab?.type === "calendar";
  const isContactsMode = currentTab?.type === "contacts";
  const isTemplatesMode = currentTab?.type === "templates";
  const isAutomationsMode = currentTab?.type === "automations";
  const isSpecialMode =
    isSettingsMode ||
    isCalendarMode ||
    isContactsMode ||
    isTemplatesMode ||
    isAutomationsMode;
  const isTimelineSidebarLayout = !isSpecialMode;
  // Navs with their own CustomSidebarHeader fill the chrome row themselves; a
  // top padding here would push the header out of it (and overflow-hidden
  // would clip a pulled-up header).
  const needsChromeRowGutter =
    isSpecialMode && !hasOwnSidebarHeaderTab(currentTab);
  return (
    <div
      className={cn([
        "flex h-full w-full shrink-0 flex-col gap-1 overflow-hidden",
        needsChromeRowGutter ? "pt-11" : "pt-0",
        !isTimelineSidebarLayout && "pr-1",
      ])}
    >
      <div className="flex flex-1 flex-col gap-1 overflow-hidden">
        {isTimelineSidebarLayout ? timelineHeader : null}
        <div className="relative min-h-0 flex-1 overflow-hidden">
          {isSettingsMode ? (
            <SettingsNav />
          ) : isCalendarMode ? (
            <CalendarNav />
          ) : isContactsMode ? (
            <ContactsNav />
          ) : isTemplatesMode ? (
            <TemplatesNav />
          ) : isAutomationsMode ? (
            <AutomationsNav />
          ) : (
            <div className="flex h-full min-h-0 flex-col">
              {noteFilter === "mine" ? (
                <>
                  {folderFilter ? (
                    <FolderMaterialsPanel folderPath={folderFilter} />
                  ) : null}
                  <div className="relative min-h-0 flex-1">
                    <TimelineView
                      folderFilter={folderFilter}
                      showIgnoredEvents={showIgnoredTimelineEvents}
                      onShowIgnoredEventsChange={
                        onShowIgnoredTimelineEventsChange
                      }
                      topChromeInset={
                        isTimelineSidebarLayout && !timelineHeader
                      }
                      topChipsOverlapHeader={
                        isTimelineSidebarLayout && !!timelineHeader
                      }
                    />
                  </div>
                </>
              ) : (
                <SharedNotesNav />
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
