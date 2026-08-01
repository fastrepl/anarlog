import type { FloatingBarSettingsChange } from "@anlg/plugin-windows";

import type { SettingValues } from "~/settings/schema";

export type LiveCaptionPosition =
  | "topCenter"
  | "topLeft"
  | "topRight"
  | "bottomLeft"
  | "bottomRight"
  | "bottomCenter";

export type FloatingOverlaySettings = {
  floatingBarOpacity: number;
  liveCaptionOpacity: number;
  liveCaptionWidth: number;
  liveCaptionLineCount: number;
  liveCaptionPosition: LiveCaptionPosition;
  liveCaptionMinimized: boolean;
};

type FloatingOverlaySettingsStorage = Pick<
  SettingValues,
  | "floating_bar_opacity"
  | "live_caption_opacity"
  | "live_caption_width"
  | "live_caption_line_count"
  | "live_caption_position"
  | "live_caption_minimized"
>;

export const DEFAULT_FLOATING_OVERLAY_SETTINGS: FloatingOverlaySettings = {
  floatingBarOpacity: 0.78,
  liveCaptionOpacity: 0.3,
  liveCaptionWidth: 440,
  liveCaptionLineCount: 1,
  liveCaptionPosition: "topCenter",
  liveCaptionMinimized: true,
};

const FLOATING_BAR_MIN_OPACITY = 0.35;
const FLOATING_BAR_MAX_OPACITY = 0.95;
const LIVE_CAPTION_MIN_OPACITY = 0.05;
const LIVE_CAPTION_MAX_OPACITY = 1;
export const LIVE_CAPTION_MIN_WIDTH = 260;
const LIVE_CAPTION_MAX_WIDTH = 640;
const LIVE_CAPTION_MIN_LINE_COUNT = 1;
const LIVE_CAPTION_MAX_LINE_COUNT = 4;

const LIVE_CAPTION_POSITIONS: ReadonlySet<string> = new Set([
  "topCenter",
  "topLeft",
  "topRight",
  "bottomLeft",
  "bottomRight",
  "bottomCenter",
]);

export const FLOATING_OVERLAY_SETTING_KEYS = [
  "floating_bar_opacity",
  "live_caption_opacity",
  "live_caption_width",
  "live_caption_line_count",
  "live_caption_position",
  "live_caption_minimized",
] as const;

export function getFloatingOverlaySettings(
  values: Partial<FloatingOverlaySettingsStorage>,
): FloatingOverlaySettings {
  return {
    floatingBarOpacity: normalizeOpacity(
      values.floating_bar_opacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.floatingBarOpacity,
      FLOATING_BAR_MIN_OPACITY,
      FLOATING_BAR_MAX_OPACITY,
    ),
    liveCaptionOpacity: normalizeOpacity(
      values.live_caption_opacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
      LIVE_CAPTION_MIN_OPACITY,
      LIVE_CAPTION_MAX_OPACITY,
    ),
    liveCaptionWidth: normalizeNumber(
      values.live_caption_width,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionWidth,
      LIVE_CAPTION_MIN_WIDTH,
      LIVE_CAPTION_MAX_WIDTH,
    ),
    liveCaptionLineCount: normalizeInteger(
      values.live_caption_line_count,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionLineCount,
      LIVE_CAPTION_MIN_LINE_COUNT,
      LIVE_CAPTION_MAX_LINE_COUNT,
    ),
    liveCaptionPosition: normalizeLiveCaptionPosition(
      values.live_caption_position,
    ),
    liveCaptionMinimized:
      (values.live_caption_minimized ??
        DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionMinimized) === true,
  };
}

function normalizeOpacity(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  return normalizeNumber(value, fallback, min, max);
}

function normalizeNumber(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }

  return Math.min(Math.max(value, min), max);
}

function normalizeInteger(
  value: unknown,
  fallback: number,
  min: number,
  max: number,
): number {
  return Math.round(normalizeNumber(value, fallback, min, max));
}

function normalizeLiveCaptionPosition(value: unknown): LiveCaptionPosition {
  if (typeof value === "string" && LIVE_CAPTION_POSITIONS.has(value)) {
    return value as LiveCaptionPosition;
  }

  return DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionPosition;
}

export function getSettingsValuesFromNativeChange(
  change: FloatingBarSettingsChange,
) {
  const values: Partial<FloatingOverlaySettingsStorage> = {};

  if (
    change.floatingBarOpacity !== null &&
    change.floatingBarOpacity !== undefined
  ) {
    values.floating_bar_opacity = normalizeOpacity(
      change.floatingBarOpacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.floatingBarOpacity,
      FLOATING_BAR_MIN_OPACITY,
      FLOATING_BAR_MAX_OPACITY,
    );
  }

  if (
    change.liveCaptionOpacity !== null &&
    change.liveCaptionOpacity !== undefined
  ) {
    values.live_caption_opacity = normalizeOpacity(
      change.liveCaptionOpacity,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionOpacity,
      LIVE_CAPTION_MIN_OPACITY,
      LIVE_CAPTION_MAX_OPACITY,
    );
  }

  if (
    change.liveCaptionWidth !== null &&
    change.liveCaptionWidth !== undefined
  ) {
    values.live_caption_width = normalizeNumber(
      change.liveCaptionWidth,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionWidth,
      LIVE_CAPTION_MIN_WIDTH,
      LIVE_CAPTION_MAX_WIDTH,
    );
  }

  if (
    change.liveCaptionLineCount !== null &&
    change.liveCaptionLineCount !== undefined
  ) {
    values.live_caption_line_count = normalizeInteger(
      change.liveCaptionLineCount,
      DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionLineCount,
      LIVE_CAPTION_MIN_LINE_COUNT,
      LIVE_CAPTION_MAX_LINE_COUNT,
    );
  }

  if (
    change.liveCaptionPosition !== null &&
    change.liveCaptionPosition !== undefined
  ) {
    values.live_caption_position = normalizeLiveCaptionPosition(
      change.liveCaptionPosition,
    );
  }

  if (
    change.liveCaptionMinimized !== null &&
    change.liveCaptionMinimized !== undefined
  ) {
    values.live_caption_minimized = change.liveCaptionMinimized === true;
  }

  return values;
}
