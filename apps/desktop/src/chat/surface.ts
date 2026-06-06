import type { ThemePreference } from "~/shared/theme/resolve";
import { resolveIsDarkMode } from "~/shared/theme/resolve";

export type ChatToolbarSurface = "light" | "dark";

export function isChatDarkAppearance(
  theme: ThemePreference,
  prefersDark: boolean,
): boolean {
  return resolveIsDarkMode(theme, prefersDark);
}

export function chatPanelClassNames(isDarkAppearance: boolean): string {
  return isDarkAppearance
    ? "bg-primary text-primary-foreground"
    : "bg-card text-foreground";
}

export function chatPanelBorderClassNames(isDarkAppearance: boolean): string {
  return isDarkAppearance ? "border-primary/80" : "border-border";
}

export function chatFloatingPanelShellClassNames(
  isDarkAppearance: boolean,
): string {
  return isDarkAppearance
    ? "bg-primary text-primary-foreground border-primary rounded-2xl border-2 shadow-[0_4px_28px_rgba(87,83,78,0.45)]"
    : "bg-card text-foreground border-border rounded-2xl border shadow-lg";
}

export function chatElevatedSurfaceClassNames(
  isDarkAppearance: boolean,
): string {
  return isDarkAppearance
    ? "bg-primary-foreground/95 text-primary"
    : "bg-muted text-foreground";
}

export function chatInputEditorClassNames(isDarkAppearance: boolean): string {
  return isDarkAppearance ? "text-primary" : "text-foreground";
}

export function chatToolbarSurface(
  isDarkAppearance: boolean,
): ChatToolbarSurface {
  return isDarkAppearance ? "dark" : "light";
}
