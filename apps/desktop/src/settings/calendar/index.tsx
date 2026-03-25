import { SyncProvider, useSync } from "~/calendar/components/context";
import { CalendarSidebarContent } from "~/calendar/components/sidebar";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

function SettingsCalendarContent() {
  const { scheduleSync } = useSync();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div className="pt-3">
      <CalendarSidebarContent />
    </div>
  );
}

export function SettingsCalendar() {
  return (
    <SyncProvider>
      <SettingsCalendarContent />
    </SyncProvider>
  );
}
