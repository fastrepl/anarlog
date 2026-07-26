import { describe, expect, test, vi } from "vitest";

import { getSessionResourcePath } from "./resource-path";

vi.mock("@tauri-apps/api/path", () => ({
  sep: vi.fn().mockReturnValue("/"),
}));

describe("getSessionResourcePath", () => {
  test("builds the resource directory for a session", () => {
    expect(getSessionResourcePath("/data/hyprnote", "session-123")).toBe(
      "/data/hyprnote/sessions/session-123",
    );
  });
});
