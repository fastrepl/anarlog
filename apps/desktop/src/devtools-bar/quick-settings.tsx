import {
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";

import { useSetSettingValues } from "~/settings/queries";
import type { SettingKey, SettingValues } from "~/settings/schema";
import { useConfigValues } from "~/shared/config";

/**
 * Settings that change what is on screen, exposed as one-click toggles so a
 * build can be checked in both states without leaving the current view. This
 * is the closest thing Anarlog has to Linear's feature-flag tiles.
 */
export const QUICK_TOGGLES = [
  { key: "floating_bar_enabled", label: "Floating bar" },
  { key: "sidebar_show_folder", label: "Sidebar folders" },
  { key: "sidebar_show_tags", label: "Sidebar tags" },
  { key: "show_tray_icon", label: "Tray icon" },
  { key: "show_app_in_dock", label: "Dock icon" },
  { key: "notification_event", label: "Event notifications" },
  { key: "notification_detect", label: "Meeting detection notifications" },
] as const satisfies ReadonlyArray<{ key: SettingKey; label: string }>;

const THEMES = ["system", "light", "dark"] as const;

const QUICK_KEYS = [...QUICK_TOGGLES.map(({ key }) => key), "theme"] as const;

// Rendered inside the dropdown so its settings subscription only lives while
// the menu is open.
export function QuickSettingsMenu() {
  const values = useConfigValues(QUICK_KEYS);
  const setValues = useSetSettingValues();

  return (
    <>
      <DropdownMenuSub>
        <DropdownMenuSubTrigger>Theme</DropdownMenuSubTrigger>
        <DropdownMenuSubContent className="w-40">
          <DropdownMenuRadioGroup
            value={values.theme}
            onValueChange={(theme) => setValues({ theme })}
          >
            {THEMES.map((theme) => (
              <DropdownMenuRadioItem
                key={theme}
                value={theme}
                className="capitalize"
                onSelect={(event) => event.preventDefault()}
              >
                {theme}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuSubContent>
      </DropdownMenuSub>
      <DropdownMenuSub>
        <DropdownMenuSubTrigger>Toggles</DropdownMenuSubTrigger>
        <DropdownMenuSubContent className="w-64">
          {QUICK_TOGGLES.map(({ key, label }) => (
            <DropdownMenuCheckboxItem
              key={key}
              checked={Boolean(values[key])}
              onCheckedChange={(checked) =>
                setValues({ [key]: checked === true } as SettingValues)
              }
              onSelect={(event) => event.preventDefault()}
            >
              {label}
            </DropdownMenuCheckboxItem>
          ))}
        </DropdownMenuSubContent>
      </DropdownMenuSub>
    </>
  );
}
