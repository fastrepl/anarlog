import { cleanup, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@anlg/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("~/chat/hooks/use-chat-appearance", () => ({
  useChatAppearance: () => ({
    isDarkAppearance: false,
    toolbarSurface: "light",
    panelClassName: "",
    panelBorderClassName: "",
    elevatedSurfaceClassName: "",
    inputEditorClassName: "",
  }),
}));

vi.mock("~/env", () => ({
  env: { VITE_APP_URL: "http://localhost:3000" },
}));

import { ErrorMessage, getChatErrorText } from "./error";

describe("ErrorMessage", () => {
  beforeEach(() => {
    cleanup();
  });

  it("renders an Error's message", () => {
    render(<ErrorMessage error={new Error("model unavailable")} />);

    expect(screen.getByText("model unavailable")).toBeTruthy();
  });

  it("renders a bare string rejection instead of crashing", () => {
    // Tauri `invoke` rejects with the serialized Rust error, which is a plain
    // string; the AI SDK stores it as `useChat().error` as-is.
    render(<ErrorMessage error="cloudsync_activity_drain_timeout" />);

    expect(screen.getByText("cloudsync_activity_drain_timeout")).toBeTruthy();
  });

  it("shows context-length help for a string error too", () => {
    render(<ErrorMessage error="prompt exceeds context length" />);

    expect(screen.getByText("Learn how to fix this")).toBeTruthy();
  });
});

describe("getChatErrorText", () => {
  it("normalizes non-Error values to a string", () => {
    expect(getChatErrorText(new Error("boom"))).toBe("boom");
    expect(getChatErrorText("boom")).toBe("boom");
    expect(getChatErrorText(42)).toBe("42");
    expect(getChatErrorText(undefined)).toBe("undefined");
  });
});
