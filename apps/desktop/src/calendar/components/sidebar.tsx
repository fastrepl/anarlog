import { platform } from "@tauri-apps/plugin-os";

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@hypr/ui/components/ui/accordion";

import { AppleCalendarSelection } from "./apple/calendar-selection";
import { AccessPermissionRow, TroubleShootingLink } from "./apple/permission";
import { SyncProvider } from "./context";
import { OAuthProviderContent } from "./oauth/provider-content";
import { PROVIDERS } from "./shared";

import { usePermission } from "~/shared/hooks/usePermissions";

export function CalendarSidebarContent() {
  const isMacos = platform() === "macos";
  const calendar = usePermission("calendar");

  const visibleProviders = PROVIDERS.filter(
    (p) => p.platform === "all" || (p.platform === "macos" && isMacos),
  );

  return (
    <SyncProvider>
      <Accordion type="single" collapsible defaultValue="apple">
        {visibleProviders.map((provider) =>
          provider.disabled ? (
            <div
              key={provider.id}
              className="flex items-center gap-2 py-2 opacity-50"
            >
              {provider.icon}
              <span className="text-sm font-medium">
                {provider.displayName}
              </span>
              {provider.badge && (
                <span className="rounded-full border border-neutral-300 px-2 text-xs font-light text-neutral-500">
                  {provider.badge}
                </span>
              )}
            </div>
          ) : (
            <AccordionItem
              key={provider.id}
              value={provider.id}
              className="border-none"
            >
              <AccordionTrigger className="py-2 hover:no-underline">
                <div className="flex items-center gap-2">
                  {provider.icon}
                  <span className="text-sm font-medium">
                    {provider.displayName}
                  </span>
                  {provider.badge && (
                    <span className="rounded-full border border-neutral-300 px-2 text-xs font-light text-neutral-500">
                      {provider.badge}
                    </span>
                  )}
                </div>
              </AccordionTrigger>
              <AccordionContent className="pb-2">
                {provider.id === "apple" && (
                  <div className="flex flex-col gap-3">
                    {calendar.status !== "authorized" ? (
                      <AccessPermissionRow
                        title="Calendar"
                        status={calendar.status}
                        isPending={calendar.isPending}
                        onOpen={calendar.open}
                        onRequest={calendar.request}
                        onReset={calendar.reset}
                      />
                    ) : (
                      <AppleCalendarSelection
                        leftAction={
                          <TroubleShootingLink
                            isPending={calendar.isPending}
                            onOpen={calendar.open}
                            onRequest={calendar.request}
                            onReset={calendar.reset}
                          />
                        }
                      />
                    )}
                  </div>
                )}
                {provider.nangoIntegrationId && (
                  <OAuthProviderContent config={provider} />
                )}
              </AccordionContent>
            </AccordionItem>
          ),
        )}
      </Accordion>
    </SyncProvider>
  );
}
