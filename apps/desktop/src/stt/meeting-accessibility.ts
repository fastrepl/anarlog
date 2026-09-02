import type { MeetingAccessibilityInspection } from "@anlg/plugin-detect";

export function inspectionShowsActiveMeeting(
  inspection: MeetingAccessibilityInspection,
): boolean {
  return Boolean(
    inspection.accessibilityTrusted &&
    inspection.platform !== "unknown" &&
    inspection.windowTitle?.trim() &&
    !inspection.warnings.some((warning) => {
      const normalized = warning.toLowerCase();
      return (
        normalized.includes("ambiguous") ||
        normalized.includes("incomplete") ||
        normalized.includes("no uniquely validated")
      );
    }),
  );
}

export function inspectionsShowActiveMeetingForApps(
  inspections: MeetingAccessibilityInspection[],
  appIds: string[],
) {
  const expectedAppIds = new Set(appIds);
  return inspections.some(
    (inspection) =>
      expectedAppIds.has(inspection.app.id) &&
      inspectionShowsActiveMeeting(inspection),
  );
}
