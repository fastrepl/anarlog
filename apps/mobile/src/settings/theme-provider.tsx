import { createContext, useContext, useMemo, type ReactNode } from "react";
import { StyleSheet, useColorScheme } from "react-native";

import { Colors, DarkColors, type ColorPalette } from "@/constants/theme";

import { usePreferences } from "./preferences";

const ThemeContext = createContext<"light" | "dark">("light");

export function ThemeProvider({ children }: { children: ReactNode }) {
  const preferences = usePreferences();
  const system = useColorScheme();
  const scheme =
    preferences.theme === "system"
      ? system === "dark"
        ? "dark"
        : "light"
      : preferences.theme;
  return (
    <ThemeContext.Provider value={scheme}>{children}</ThemeContext.Provider>
  );
}

export function useAppColorScheme() {
  return useContext(ThemeContext);
}
export function useColors(): ColorPalette {
  return useAppColorScheme() === "dark" ? DarkColors : Colors;
}

export function createStyleHook<T extends StyleSheet.NamedStyles<T>>(
  factory: (colors: ColorPalette) => T,
) {
  return function useStyles(): T {
    const colors = useColors();
    return useMemo(() => StyleSheet.create(factory(colors)), [colors]);
  };
}
