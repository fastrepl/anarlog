import * as stylex from "@stylexjs/stylex";
import { type ReactNode } from "react";

import { AutomationsNav } from "./automations";
import { CalendarNav } from "./calendar";
import { ContactsNav } from "./contacts";
import type { SidebarNoteFilter } from "./note-filter";
import { SettingsNav } from "./settings";
import { SharedNotesNav } from "./shared-notes";
import { TemplatesNav } from "./templates";
import { TimelineView } from "./timeline";
import { hasOwnSidebarHeaderTab } from "./use-custom-sidebar";

import { useTabs } from "~/store/zustand/tabs";

export function LeftSidebar({
  noteFilter = "mine",
  timelineHeader,
  showIgnoredTimelineEvents,
  onShowIgnoredTimelineEventsChange,
}: {
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
      {...stylex.props(
        styles.root,
        needsChromeRowGutter ? styles.chromeGutter : styles.noChromeGutter,
        !isTimelineSidebarLayout && styles.specialMode,
      )}
    >
      <div {...stylex.props(styles.content)}>
        {isTimelineSidebarLayout ? timelineHeader : null}
        <div {...stylex.props(styles.nav)}>
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
            <div {...stylex.props(styles.timeline)}>
              {noteFilter === "mine" ? (
                <div {...stylex.props(styles.timelineContent)}>
                  <TimelineView
                    showIgnoredEvents={showIgnoredTimelineEvents}
                    onShowIgnoredEventsChange={
                      onShowIgnoredTimelineEventsChange
                    }
                    topChromeInset={isTimelineSidebarLayout && !timelineHeader}
                    topChipsOverlapHeader={
                      isTimelineSidebarLayout && !!timelineHeader
                    }
                  />
                </div>
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

const styles = stylex.create({
  chromeGutter: {
    paddingTop: "2.75rem",
  },
  content: {
    display: "flex",
    flex: "1",
    flexDirection: "column",
    gap: "0.25rem",
    overflow: "hidden",
  },
  nav: {
    flex: "1",
    minHeight: 0,
    overflow: "hidden",
    position: "relative",
  },
  noChromeGutter: {
    paddingTop: 0,
  },
  root: {
    display: "flex",
    flexDirection: "column",
    flexShrink: 0,
    gap: "0.25rem",
    height: "100%",
    overflow: "hidden",
    width: "100%",
  },
  specialMode: {
    paddingRight: "0.25rem",
  },
  timeline: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    minHeight: 0,
  },
  timelineContent: {
    flex: "1",
    minHeight: 0,
    position: "relative",
  },
});
