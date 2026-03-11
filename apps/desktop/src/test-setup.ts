import { randomUUID } from "node:crypto";
import { vi } from "vitest";

Object.defineProperty(globalThis.crypto, "randomUUID", { value: randomUUID });

Object.defineProperty(globalThis, "window", {
  value: {
    ...globalThis.window,
    __TAURI_INTERNALS__: {
      metadata: {
        currentWindow: {
          label: "main",
        },
        currentWebview: {
          label: "main",
        },
      },
    },
  },
  writable: true,
});

vi.mock("@tauri-apps/api/path", () => ({
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setProperties: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setDisabled: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    isDisabled: vi.fn().mockResolvedValue({ status: "ok", data: false }),
  },
}));

vi.mock("./types/tauri.gen", () => ({
  commands: {
    getRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
    setRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
  },
}));
