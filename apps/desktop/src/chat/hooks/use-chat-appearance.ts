import { useSyncExternalStore } from "react";

import {
  chatElevatedSurfaceClassNames,
  chatInputEditorClassNames,
  chatPanelBorderClassNames,
  chatPanelClassNames,
  chatToolbarSurface,
  isChatDarkAppearance,
} from "~/chat/surface";
import { useConfigValue } from "~/shared/config";
import type { ThemePreference } from "~/shared/theme/resolve";

function subscribePrefersDark(onStoreChange: () => void): () => void {
  const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  mediaQuery.addEventListener("change", onStoreChange);
  return () => mediaQuery.removeEventListener("change", onStoreChange);
}

function getPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function useChatAppearance() {
  const theme = useConfigValue("theme") as ThemePreference;
  const prefersDark = useSyncExternalStore(
    subscribePrefersDark,
    getPrefersDark,
    () => false,
  );
  const isDarkAppearance = isChatDarkAppearance(theme, prefersDark);

  return {
    isDarkAppearance,
    toolbarSurface: chatToolbarSurface(isDarkAppearance),
    panelClassName: chatPanelClassNames(isDarkAppearance),
    panelBorderClassName: chatPanelBorderClassNames(isDarkAppearance),
    elevatedSurfaceClassName:
      chatElevatedSurfaceClassNames(isDarkAppearance),
    inputEditorClassName: chatInputEditorClassNames(isDarkAppearance),
  };
}
