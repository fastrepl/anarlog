import { CalendarSidebarContent } from "~/calendar/components/sidebar";

export function CalendarNav() {
  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <div className="flex h-12 shrink-0 items-center py-2 pr-1 pl-3">
        <h3 className="font-serif text-sm font-medium">Calendar</h3>
      </div>
      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto px-3">
        <CalendarSidebarContent />
      </div>
    </div>
  );
}
