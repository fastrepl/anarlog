import { useSettingsReady } from "~/settings/queries";

export function useSettingsThemeReady(): boolean {
  return useSettingsReady();
}
