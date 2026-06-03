import { describe, expect, it } from "vitest";

import {
  hasSessionContextDragData,
  readSessionContextDragData,
  readSessionMentionDragData,
  writeSessionContextDragData,
} from "./session-drag";

class FakeDataTransfer {
  effectAllowed = "all";
  private readonly values = new Map<string, string>();

  get types() {
    return Array.from(this.values.keys());
  }

  getData(type: string) {
    return this.values.get(type) ?? "";
  }

  setData(type: string, value: string) {
    this.values.set(type, value);
  }
}

describe("session drag context", () => {
  it("writes and reads manual session context refs", () => {
    const dataTransfer = new FakeDataTransfer() as unknown as DataTransfer;

    writeSessionContextDragData(dataTransfer, "session-1", "Meeting notes");

    expect(dataTransfer.effectAllowed).toBe("copy");
    expect(hasSessionContextDragData(dataTransfer)).toBe(true);
    expect(dataTransfer.getData("text/plain")).toBe("Meeting notes");
    expect(readSessionMentionDragData(dataTransfer)).toEqual({
      id: "session-1",
      label: "Meeting notes",
    });
    expect(readSessionContextDragData(dataTransfer)).toEqual({
      kind: "session",
      key: "session:manual:session-1",
      source: "manual",
      sessionId: "session-1",
    });
  });

  it("uses an untitled chip label for legacy session drag payloads", () => {
    const dataTransfer = new FakeDataTransfer() as unknown as DataTransfer;

    dataTransfer.setData(
      "application/x-anarlog-session-context",
      JSON.stringify({ sessionId: "session-1" }),
    );

    expect(readSessionMentionDragData(dataTransfer)).toEqual({
      id: "session-1",
      label: "Untitled",
    });
  });

  it("ignores malformed session drag payloads", () => {
    const dataTransfer = new FakeDataTransfer() as unknown as DataTransfer;

    dataTransfer.setData("application/x-anarlog-session-context", "{");

    expect(readSessionContextDragData(dataTransfer)).toBeNull();
  });

  it("ignores non-session drops", () => {
    const dataTransfer = new FakeDataTransfer() as unknown as DataTransfer;

    dataTransfer.setData("text/plain", "Meeting notes");

    expect(hasSessionContextDragData(dataTransfer)).toBe(false);
    expect(readSessionContextDragData(dataTransfer)).toBeNull();
  });
});
