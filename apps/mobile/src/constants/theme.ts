import {
  DesignColors,
  DesignControl,
  DesignRadius,
  DesignSpacing,
  DesignTypography,
} from "@anlg/design-system";

function palette(light: (typeof DesignColors)[keyof typeof DesignColors]) {
  return {
    background: light.background,
    brandBackgroundTop: "#F7E7B0",
    brandBackgroundBottom: "#FFE08C",
    paper: light.background,
    surface: light.card,
    ink: light.foreground,
    inkInverse: light.primaryForeground,
    primary: light.primary,
    primaryForeground: light.primaryForeground,
    secondary: light.secondary,
    mutedSurface: light.muted,
    muted: light.mutedForeground,
    accentSurface: light.accent,
    border: light.border,
    accent: light.destructive,
    destructive: light.destructive,
    destructiveForeground: light.destructiveForeground,
    alert: light.alert,
    alertForeground: light.alertForeground,
    alertBorder: light.alertBorder,
    scrim: light.scrim,
  } as const;
}

export const Colors = palette(DesignColors.light);
export const DarkColors = palette(DesignColors.dark);
export type ColorPalette = { [K in keyof typeof Colors]: string };

export const Gradients = {
  brandBackground: "linear-gradient(to bottom, #F7E7B0 0%, #FFE08C 100%)",
} as const;

export const Spacing = {
  xs: DesignSpacing.xs,
  sm: DesignSpacing.sm,
  md: DesignSpacing.lg,
  lg: DesignSpacing.xl,
  xl: DesignSpacing.xxl,
  compact: DesignSpacing.md,
  roomy: DesignSpacing.xxxl,
} as const;

export const Radius = {
  card: DesignRadius.xl,
  control: DesignRadius.lg,
  panel: DesignRadius.panel,
  sheet: DesignRadius.sheet,
  pill: DesignRadius.pill,
} as const;

export const CornerCurve = {
  squircle: "continuous",
} as const;

export const Typography = DesignTypography;
export const ControlSize = DesignControl;

export const LISTENING_CONTROL_HEIGHT = DesignControl.listening;
export const LISTENING_CONTROL_RADIUS = DesignRadius.panel;
