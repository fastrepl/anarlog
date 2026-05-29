import { CustomSidebarHeader } from "./custom-sidebar-header";

import { CalendarSidebarContent } from "~/calendar/components/sidebar";

export function CalendarNav() {
  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <CustomSidebarHeader title="Calendar" />
      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto px-3">
        <CalendarSidebarContent />
      </div>
    </div>
  );
}
