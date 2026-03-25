import { SyncProvider } from "~/calendar/components/context";
import { CalendarSidebarContent } from "~/calendar/components/sidebar";

export function SettingsCalendar() {
  return (
    <SyncProvider>
      <div className="pt-3">
        <CalendarSidebarContent />
      </div>
    </SyncProvider>
  );
}
