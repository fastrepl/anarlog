import { CalendarView } from "./components/calendar-view";

import { StandardTabWrapper } from "~/shared/main";

export function TabContentCalendar() {
  return (
    <StandardTabWrapper>
      <CalendarView />
    </StandardTabWrapper>
  );
}
