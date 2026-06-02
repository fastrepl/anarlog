import { describe, expect, it } from "vitest";

import {
  hasSessionContextDragData,
  readSessionContextDragData,
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
    expect(readSessionContextDragData(dataTransfer)).toEqual({
      kind: "session",
      key: "session:manual:session-1",
      source: "manual",
      sessionId: "session-1",
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
