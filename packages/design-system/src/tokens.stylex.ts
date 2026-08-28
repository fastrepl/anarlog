import * as stylex from "@stylexjs/stylex";

export const colors = stylex.defineVars({
  accent: "hsl(var(--accent, 60 5% 94%))",
  accentForeground: "hsl(var(--accent-foreground, 24 10% 10%))",
  alert: "hsl(var(--alert, 0 86% 97%))",
  alertBorder: "hsl(var(--alert-border, 0 90% 86%))",
  alertForeground: "hsl(var(--alert-foreground, 0 72% 42%))",
  appFloatingBorder: "hsl(var(--app-floating-border, 24 6% 86%))",
  appFloatingChrome: "hsl(var(--app-floating-chrome, 60 5% 94%))",
  appFloatingPanel: "hsl(var(--app-floating-panel, 60 9% 98%))",
  background: "hsl(var(--background, 60 9% 98%))",
  border: "hsl(var(--border, 24 6% 90%))",
  card: "hsl(var(--card, 0 0% 100%))",
  cardForeground: "hsl(var(--card-foreground, 24 10% 10%))",
  destructive: "hsl(var(--destructive, 0 84.2% 60.2%))",
  destructiveForeground: "hsl(var(--destructive-foreground, 0 0% 98%))",
  foreground: "hsl(var(--foreground, 24 10% 10%))",
  input: "hsl(var(--input, 24 6% 90%))",
  muted: "hsl(var(--muted, 60 5% 96%))",
  mutedForeground: "hsl(var(--muted-foreground, 25 5% 45%))",
  popover: "hsl(var(--popover, 0 0% 100%))",
  popoverForeground: "hsl(var(--popover-foreground, 24 10% 10%))",
  primary: "hsl(var(--primary, 24 10% 16%))",
  primaryForeground: "hsl(var(--primary-foreground, 60 9% 98%))",
  ring: "hsl(var(--ring, 24 10% 10%))",
  secondary: "hsl(var(--secondary, 60 5% 96%))",
  secondaryForeground: "hsl(var(--secondary-foreground, 24 10% 10%))",
  sidebar: "hsl(var(--sidebar-background, 60 9% 98%))",
  sidebarAccent: "hsl(var(--sidebar-accent, 60 5% 90%))",
  sidebarAccentForeground: "hsl(var(--sidebar-accent-foreground, 24 10% 10%))",
  sidebarBorder: "hsl(var(--sidebar-border, 24 6% 90%))",
  sidebarForeground: "hsl(var(--sidebar-foreground, 24 10% 10%))",
  sidebarPrimary: "hsl(var(--sidebar-primary, 24 10% 16%))",
  sidebarPrimaryForeground: "hsl(var(--sidebar-primary-foreground, 60 9% 98%))",
  sidebarRing: "hsl(var(--sidebar-ring, 24 10% 10%))",
});

export const fonts = stylex.defineConsts({
  hand: "var(--font-hand, 'Bradley Hand', 'Segoe Print', cursive)",
  mono: "var(--font-mono, ui-monospace, 'SFMono-Regular', Consolas, monospace)",
  sans: "var(--font-sans, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif)",
});

export const media = stylex.defineConsts({
  md: "@media (min-width: 48rem)",
  reducedMotion: "@media (prefers-reduced-motion: reduce)",
  sm: "@media (min-width: 40rem)",
});

export const radii = stylex.defineConsts({
  full: "9999px",
  lg: "var(--radius, 0.5rem)",
  md: "calc(var(--radius, 0.5rem) - 2px)",
  sm: "calc(var(--radius, 0.5rem) - 4px)",
  xl: "calc(var(--radius, 0.5rem) + 4px)",
});

export const shadows = stylex.defineConsts({
  lg: "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)",
  sm: "0 1px 2px 0 rgb(0 0 0 / 0.05)",
});

export const spacing = stylex.defineConsts({
  lg: "1rem",
  md: "0.75rem",
  none: "0",
  sm: "0.5rem",
  xl: "1.5rem",
  xs: "0.25rem",
  xxl: "2rem",
});
