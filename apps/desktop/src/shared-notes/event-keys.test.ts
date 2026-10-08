import { describe, expect, it } from "vitest";

import { sharedNoteEventKey } from "./event-key";
import {
  parseSharedNoteEventKeys,
  sharedNoteIdsForEventKey,
} from "./event-keys";

describe("shared note event keys", () => {
  it("matches a recipient's calendar event regardless of timestamp format", () => {
    const shared = sharedNoteEventKey("ical-uid", "2026-10-08T15:00:00Z");
    const local = sharedNoteEventKey("ical-uid", "2026-10-08T17:00:00+02:00");
    expect(local).toBe(shared);
    expect(sharedNoteEventKey("", "2026-10-08T15:00:00Z")).toBe("");
    expect(sharedNoteEventKey("ical-uid", "not a date")).toBe("");
  });

  it("only exposes keys stored for the signed-in viewer", () => {
    const valueJson = JSON.stringify({
      viewer_user_id: "viewer",
      keys: { share: "ical-uid|2026-10-08T15:00:00.000Z" },
    });
    const keys = parseSharedNoteEventKeys(valueJson, "viewer");
    expect(keys.get("share")).toBe("ical-uid|2026-10-08T15:00:00.000Z");
    expect(parseSharedNoteEventKeys(valueJson, "other").size).toBe(0);
    expect(parseSharedNoteEventKeys("{", "viewer").size).toBe(0);
  });

  it("merges only received shares for the same event into a note", () => {
    const key = "ical-uid|2026-10-08T15:00:00.000Z";
    const keys = new Map([
      ["received", key],
      ["owned", key],
      ["other-event", "ical-uid|2026-10-15T15:00:00.000Z"],
    ]);
    const notes = [
      { shareId: "received", manageAccess: false },
      { shareId: "owned", manageAccess: true },
      { shareId: "other-event", manageAccess: false },
    ];
    expect(sharedNoteIdsForEventKey(key, keys, notes)).toEqual(["received"]);
    expect(sharedNoteIdsForEventKey("", keys, notes)).toEqual([]);
  });
});
