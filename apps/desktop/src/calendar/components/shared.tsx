import { Icon } from "@iconify-icon/react";
import type { ReactNode } from "react";

import { OutlookIcon } from "@hypr/ui/components/icons/outlook";

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
    docsPath:
      "https://char.com/docs/calendar/apple?utm_source=app&utm_medium=settings&utm_campaign=organic",
    nangoIntegrationId: undefined,
  },
  {
    disabled: false,
    id: "google",
    displayName: "Google",
    badge: "Beta",
    icon: <Icon icon="logos:google-calendar" width={20} height={20} />,
    platform: "all",
    docsPath:
      "https://char.com/docs/calendar/gcal?utm_source=app&utm_medium=settings&utm_campaign=organic",
    nangoIntegrationId: "google-calendar",
  },
  {
    disabled: false,
    id: "outlook",
    displayName: "Outlook",
    badge: "Beta",
    icon: <OutlookIcon size={20} />,
    platform: "all",
    docsPath:
      "https://char.com/docs/calendar/outlook?utm_source=app&utm_medium=settings&utm_campaign=organic",
    nangoIntegrationId: "outlook",
  },
] as const satisfies readonly CalendarProvider[];

export const PROVIDERS = [..._PROVIDERS];
