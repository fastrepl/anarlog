import { describe, expect, it } from "vitest";

import { shouldShowNotificationFromSettings } from "./notification-policy";

function stored(values: Record<string, boolean>) {
  return {
    values,
    hasValues: new Set(Object.keys(values)),
  } as Parameters<typeof shouldShowNotificationFromSettings>[0];
}

describe("shouldShowNotificationFromSettings", () => {
  it("shows notification categories by default", () => {
    expect(
      shouldShowNotificationFromSettings(
        stored({}),
        "notification_transcription_complete",
      ),
    ).toBe(true);
  });

  it("honors a disabled notification category", () => {
    expect(
      shouldShowNotificationFromSettings(
        stored({ notification_summary_complete: false }),
        "notification_summary_complete",
      ),
    ).toBe(false);
  });

  it("lets the master switch suppress every category", () => {
    expect(
      shouldShowNotificationFromSettings(
        stored({
          notification_disabled: true,
          notification_cloudsync_complete: true,
        }),
        "notification_cloudsync_complete",
      ),
    ).toBe(false);
  });
});
