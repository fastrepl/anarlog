import {
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@anlg/ui/components/ui/dropdown-menu";

import { MenuGroup, MenuHint } from "./menu";

import { useSetSettingValues } from "~/settings/queries";
import type { SettingKey, SettingValues } from "~/settings/schema";
import { useConfigValues } from "~/shared/config";

/**
 * Settings that change what is on screen, exposed as one-click toggles so a
 * build can be checked in both states without leaving the current view. This
 * is the closest thing Anarlog has to Linear's feature-flag tiles.
 */
export const QUICK_TOGGLES = [
  {
    key: "floating_bar_enabled",
    label: "Floating bar",
    description: "Show the floating recording bar while a meeting is active.",
  },
  {
    key: "sidebar_show_folder",
    label: "Sidebar folders",
    description: "Show the Folders section in the sidebar.",
  },
  {
    key: "sidebar_show_tags",
    label: "Sidebar tags",
    description: "Show the Tags section in the sidebar.",
  },
  {
    key: "show_tray_icon",
    label: "Tray icon",
    description: "Show the menu bar / system tray icon.",
  },
  {
    key: "show_app_in_dock",
    label: "Dock icon",
    description: "Keep the app icon in the Dock while running (macOS).",
  },
  {
    key: "notification_event",
    label: "Event notifications",
    description: "Notify before a calendar event starts.",
  },
  {
    key: "notification_detect",
    label: "Meeting detection notifications",
    description: "Notify when a meeting app starts using the microphone.",
  },
] as const satisfies ReadonlyArray<{
  key: SettingKey;
  label: string;
  description: string;
}>;

const THEMES = [
  {
    value: "system",
    label: "System",
    description: "Follow the OS appearance.",
  },
  {
    value: "light",
    label: "Light",
    description: "Always use the light theme.",
  },
  { value: "dark", label: "Dark", description: "Always use the dark theme." },
] as const;

const QUICK_KEYS = [...QUICK_TOGGLES.map(({ key }) => key), "theme"] as const;

// Rendered inside the dropdown so its settings subscription only lives while
// the menu is open.
export function QuickSettingsMenu() {
  const values = useConfigValues(QUICK_KEYS);
  const setValues = useSetSettingValues();

  return (
    <>
      <MenuGroup
        label="Theme"
        description="Switch the app theme without opening Settings."
        className="w-44"
      >
        <DropdownMenuRadioGroup
          value={values.theme}
          onValueChange={(theme) => setValues({ theme })}
        >
          {THEMES.map((theme) => (
            <MenuHint key={theme.value} description={theme.description}>
              <DropdownMenuRadioItem
                value={theme.value}
                onSelect={(event) => event.preventDefault()}
              >
                {theme.label}
              </DropdownMenuRadioItem>
            </MenuHint>
          ))}
        </DropdownMenuRadioGroup>
      </MenuGroup>
      <MenuGroup
        label="Toggles"
        description="Flip display-affecting settings in place to compare both states."
        className="w-64"
      >
        {QUICK_TOGGLES.map(({ key, label, description }) => (
          <MenuHint key={key} description={description}>
            <DropdownMenuCheckboxItem
              checked={Boolean(values[key])}
              onCheckedChange={(checked) =>
                setValues({ [key]: checked === true } as SettingValues)
              }
              onSelect={(event) => event.preventDefault()}
            >
              {label}
            </DropdownMenuCheckboxItem>
          </MenuHint>
        ))}
      </MenuGroup>
    </>
  );
}
