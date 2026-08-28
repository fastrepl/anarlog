import { Icon } from "@iconify-icon/react";
import type { ReactNode } from "react";

import { OutlookIcon } from "@anlg/ui/components/icons/outlook";

export type CalendarProvider = {
  disabled: boolean;
  id: string;
  displayName: string;
  icon: ReactNode;
  badge?: string | null;
  platform?: "macos" | "all";
  docsPath: string;
  nangoIntegrationId?: string;
};

const _PROVIDERS = [
  {
    disabled: false,
    id: "apple",
    displayName: "Apple Calendar",
    badge: "",
    icon: (
      <img
        src="/assets/apple-calendar.png"
        alt="Apple Calendar"
        className="size-5 rounded-[4px] object-cover"
      />
    ),
    platform: "macos",
    docsPath: "https://docs.anarlog.so/calendar#apple-calendar",
    nangoIntegrationId: undefined,
  },
  {
    disabled: false,
    id: "google",
    displayName: "Google",
    badge: "",
    icon: <Icon icon="logos:google-calendar" width={16} height={16} />,
    platform: "all",
    docsPath: "https://docs.anarlog.so/calendar#google-calendar",
    nangoIntegrationId: "google-calendar",
  },
  {
    disabled: false,
    id: "outlook",
    displayName: "Outlook",
    badge: "",
    icon: <OutlookIcon size={16} />,
    platform: "all",
    docsPath: "https://docs.anarlog.so/calendar#outlook-calendar",
    nangoIntegrationId: "outlook",
  },
] as const satisfies readonly CalendarProvider[];

export const PROVIDERS = [..._PROVIDERS];

const CALENDAR_NANGO_INTEGRATION_IDS = new Set<string>(
  PROVIDERS.flatMap((provider) =>
    provider.nangoIntegrationId ? [provider.nangoIntegrationId] : [],
  ),
);

export function getCalendarConnectionKey(
  connections:
    | Array<{ connection_id: string; integration_id: string }>
    | undefined,
) {
  return (connections ?? [])
    .filter((connection) =>
      CALENDAR_NANGO_INTEGRATION_IDS.has(connection.integration_id),
    )
    .map((connection) => connection.connection_id)
    .sort()
    .join(",");
}
