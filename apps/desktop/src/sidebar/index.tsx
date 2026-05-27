import { lazy, Suspense } from "react";

import { CalendarNav } from "./calendar";
import { ContactsNav } from "./contacts";
import { SettingsNav } from "./settings";
import { TemplatesNav } from "./templates";
import { TimelineView } from "./timeline";
import { ToastArea } from "./toast";

import { useShell } from "~/contexts/shell";
import { useTabs } from "~/store/zustand/tabs";

const DevtoolView = lazy(() =>
  import("./devtool").then((m) => ({ default: m.DevtoolView })),
);

export function LeftSidebar() {
  const { leftsidebar } = useShell();
  const currentTab = useTabs((state) => state.currentTab);

  const isSettingsMode = currentTab?.type === "settings";
  const isCalendarMode = currentTab?.type === "calendar";
  const isContactsMode = currentTab?.type === "contacts";
  const isTemplatesMode = currentTab?.type === "templates";
  const isSpecialMode =
    isSettingsMode || isCalendarMode || isContactsMode || isTemplatesMode;

  return (
    <div className="flex h-full w-[200px] shrink-0 flex-col gap-1 overflow-hidden pt-1">
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
            <TimelineView />
          )}
          {!leftsidebar.showDevtool && !isSpecialMode && <ToastArea />}
        </div>
      </div>
    </div>
  );
}
