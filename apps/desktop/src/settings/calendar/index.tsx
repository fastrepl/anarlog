import { SyncProvider, useSync } from "~/calendar/components/context";
import { CalendarSidebarContent } from "~/calendar/components/sidebar";
import { SettingsPageTitle } from "~/settings/page-title";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

function SettingsCalendarContent() {
  const { scheduleSync } = useSync();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <div className="flex flex-col gap-4">
      <SettingsPageTitle title="Calendar" />
      <CalendarSidebarContent returnTo="settings-calendar" />
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
