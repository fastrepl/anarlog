import { platform } from "@tauri-apps/plugin-os";
import { CalendarIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { OnboardingButton } from "./shared";

import { useAppleCalendarSelection } from "~/calendar/components/apple/calendar-selection";
import { ApplePermissions } from "~/calendar/components/apple/permission";
import { CalendarSelection } from "~/calendar/components/calendar-selection";
import { SyncProvider } from "~/calendar/components/context";
import { OAuthProviderContent } from "~/calendar/components/oauth/provider-content";
import { PROVIDERS, type CalendarProvider } from "~/calendar/components/shared";
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

function CalendarProviderCard({
  provider,
  children,
}: {
  provider: CalendarProvider;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-3 rounded-xl border border-neutral-200 bg-white/65 p-4 backdrop-blur-[2px]">
      <div className="flex items-center gap-2">
        <div className="shrink-0">{provider.icon}</div>
        <div className="min-w-0">
          <h3 className="text-sm font-medium text-neutral-900">
            {provider.displayName} Calendar
          </h3>
          <p className="text-xs text-neutral-500">
            {provider.id === "apple"
              ? "Use calendars from the Calendar app on this Mac"
              : "Connect a Google account and choose which calendars to sync"}
          </p>
        </div>
      </div>
      {children}
    </div>
  );
}

function AppleCalendarSetup() {
  const calendar = usePermission("calendar");
  const isAuthorized = calendar.status === "authorized";
  const appleProvider = PROVIDERS.find((provider) => provider.id === "apple");

  if (!appleProvider) return null;

  return (
    <CalendarProviderCard provider={appleProvider}>
      <ApplePermissions />

      {isAuthorized ? (
        <AppleCalendarList />
      ) : (
        <RequestCalendarAccess
          onRequest={calendar.request}
          isPending={calendar.isPending}
        />
      )}
    </CalendarProviderCard>
  );
}

function OAuthCalendarSetup({ providerId }: { providerId: "google" }) {
  const provider = PROVIDERS.find((item) => item.id === providerId);

  if (!provider) return null;

  return (
    <CalendarProviderCard provider={provider}>
      <div
        className={cn([
          "rounded-lg border border-neutral-200 bg-stone-50/60 px-4 py-3",
          "text-sm text-neutral-700",
        ])}
      >
        <OAuthProviderContent config={provider} />
      </div>
    </CalendarProviderCard>
  );
}

export function CalendarSection({ onContinue }: { onContinue: () => void }) {
  const isMacos = platform() === "macos";

  return (
    <SyncProvider>
      <div className="flex flex-col gap-4">
        <div
          className={cn([
            "grid gap-3",
            isMacos ? "xl:grid-cols-2" : "grid-cols-1",
          ])}
        >
          {isMacos && <AppleCalendarSetup />}
          <OAuthCalendarSetup providerId="google" />
        </div>

        <OnboardingButton onClick={onContinue}>Continue</OnboardingButton>
      </div>
    </SyncProvider>
  );
}
