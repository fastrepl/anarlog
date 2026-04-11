import { Icon } from "@iconify-icon/react";
import type { ReactNode } from "react";

import { OutlookIcon } from "@hypr/ui/components/icons/outlook";
import { withCharUtm } from "@hypr/utils";

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
    docsPath: withCharUtm("https://char.com/docs/calendar/apple", {
      source: "app",
      medium: "settings",
    }),
    nangoIntegrationId: undefined,
  },
  {
    disabled: false,
    id: "google",
    displayName: "Google",
    badge: "Beta",
    icon: <Icon icon="logos:google-calendar" width={20} height={20} />,
    platform: "all",
    docsPath: withCharUtm("https://char.com/docs/calendar/gcal", {
      source: "app",
      medium: "settings",
    }),
    nangoIntegrationId: "google-calendar",
  },
  {
    disabled: false,
    id: "outlook",
    displayName: "Outlook",
    badge: "Beta",
    icon: <OutlookIcon size={20} />,
    platform: "all",
    docsPath: withCharUtm("https://char.com/docs/calendar/outlook", {
      source: "app",
      medium: "settings",
    }),
    nangoIntegrationId: "outlook",
  },
] as const satisfies readonly CalendarProvider[];

export const PROVIDERS = [..._PROVIDERS];
