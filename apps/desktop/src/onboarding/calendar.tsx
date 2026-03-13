import { platform } from "@tauri-apps/plugin-os";
import { CalendarIcon } from "lucide-react";

import { OnboardingButton } from "./shared";

import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { ApplePermissions } from "~/calendar/components/apple/permission";
import { CalendarSelection } from "~/calendar/components/calendar-selection";
import { SyncProvider } from "~/calendar/components/context";
import { usePermission } from "~/shared/hooks/usePermissions";
import * as main from "~/store/tinybase/store/main";

function AppleCalendarList() {
  const { groups, handleToggle, isLoading } = useAppleCalendarSelection();
  return (
    <CalendarSelection
      groups={groups}
      onToggle={handleToggle}
      isLoading={isLoading}
      className="rounded-lg border"
    />
  );
}

function RequestCalendarAccess({
  onRequest,
  isPending,
}: {
  onRequest: () => void;
  isPending: boolean;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border px-4 py-6">
      <CalendarIcon className="mb-2 size-6 text-neutral-300" />
      <OnboardingButton
        onClick={onRequest}
        disabled={isPending}
        className="border-stone-700 bg-stone-800 text-white hover:bg-stone-700"
      >
        Request Access to Apple Calendar
      </OnboardingButton>
    </div>
  );
}

export function CalendarSection({ onContinue }: { onContinue: () => void }) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const isAuthorized = calendar.status === "authorized";
  const enabledCalendars = main.UI.useResultTable(
    main.QUERIES.enabledCalendars,
    main.STORE_ID,
  );
  const hasConnectedCalendar = Object.keys(enabledCalendars ?? {}).length > 0;

  return (
    <div className="flex flex-col gap-4">
      {isMacos && (
        <div className="flex flex-col gap-4">
          <ApplePermissions />

          {isAuthorized ? (
            <SyncProvider>
              <AppleCalendarList />
            </SyncProvider>
          ) : (
            <RequestCalendarAccess
              onRequest={calendar.request}
              isPending={calendar.isPending}
            />
          )}
        </div>
      )}

      {hasConnectedCalendar ? (
        <OnboardingButton onClick={onContinue}>Continue</OnboardingButton>
      ) : (
        <button
          type="button"
          onClick={onContinue}
          className="w-fit text-sm text-neutral-500/70 transition-colors hover:text-neutral-700"
        >
          Skip
        </button>
      )}
    </div>
  );
}
