import { platform } from "@tauri-apps/plugin-os";

import { OnboardingButton } from "./shared";

import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { TroubleShootingLink } from "~/calendar/components/apple/permission";
import { CalendarSelection } from "~/calendar/components/calendar-selection";
import { SyncProvider, useSync } from "~/calendar/components/context";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { usePermission } from "~/shared/hooks/usePermissions";
import * as main from "~/store/tinybase/store/main";

function AppleCalendarList() {
  const { scheduleSync } = useSync();
  const { groups, handleToggle, isLoading } = useAppleCalendarSelection();

  useMountEffect(() => {
    scheduleSync();
  });

  return (
    <CalendarSelection
      groups={groups}
      onToggle={handleToggle}
      isLoading={isLoading}
      disableHoverTone
      className="rounded-xl border border-white/45 bg-white/28 shadow-[inset_0_1px_0_rgba(255,255,255,0.4),0_8px_24px_-20px_rgba(87,83,78,0.35)] backdrop-blur-md backdrop-saturate-150"
    />
  );
}

function AppleCalendarProvider({
  isAuthorized,
  isPending,
  onRequest,
  onOpen,
  onReset,
}: {
  isAuthorized: boolean;
  isPending: boolean;
  onRequest: () => void;
  onOpen: () => void;
  onReset: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      {isAuthorized ? (
        <SyncProvider>
          <AppleCalendarList />
        </SyncProvider>
      ) : (
        <div className="flex items-center gap-3">
          <OnboardingButton
            onClick={onRequest}
            disabled={isPending}
            className="flex items-center gap-3 border border-neutral-200 bg-white text-stone-800 shadow-[0_2px_6px_rgba(87,83,78,0.08),0_10px_18px_-10px_rgba(87,83,78,0.22)] hover:bg-stone-50"
          >
            <img
              src="/assets/apple-calendar.png"
              alt=""
              aria-hidden="true"
              className="size-5 rounded-[4px] object-cover"
            />
            Connect Apple Calendar
          </OnboardingButton>
          <TroubleShootingLink
            onRequest={onRequest}
            onReset={onReset}
            onOpen={onOpen}
            isPending={isPending}
            className="text-sm text-neutral-500"
          />
        </div>
      )}
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
  const hasConnectedCalendar =
    isAuthorized && Object.keys(enabledCalendars ?? {}).length > 0;

  return (
    <div className="flex flex-col gap-4">
      {isMacos && (
        <AppleCalendarProvider
          isAuthorized={isAuthorized}
          isPending={calendar.isPending}
          onRequest={calendar.request}
          onOpen={calendar.open}
          onReset={calendar.reset}
        />
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
