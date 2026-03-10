import { platform } from "@tauri-apps/plugin-os";
import { CalendarIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";

import { OnboardingButton } from "./shared";

import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { ApplePermissions } from "~/calendar/components/apple/permission";
import { CalendarSelection } from "~/calendar/components/calendar-selection";
import { SyncProvider } from "~/calendar/components/context";
import { usePermission } from "~/shared/hooks/usePermissions";

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
      <Button
        variant="outline"
        size="sm"
        onClick={onRequest}
        disabled={isPending}
      >
        Request Access to Calendar
      </Button>
    </div>
  );
}

export function CalendarSection({ onContinue }: { onContinue: () => void }) {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");
  const isAuthorized = calendar.status === "authorized";

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

      <OnboardingButton onClick={onContinue}>Continue</OnboardingButton>
    </div>
  );
}
