import { describe, expect, it } from "vitest";

import { parseIncomingMessage } from "../../entrypoints/shared/host-message";

describe("parseIncomingMessage", () => {
  it("parses valid meeting_state payload", () => {
    const message = parseIncomingMessage({
      type: "meeting_state",
      url: "https://meet.google.com/abc-defg-hij",
      is_active: true,
      muted: false,
      participants: [{ name: "Alice", is_self: true }],
    });

    expect(message).toEqual({
      type: "meeting_state",
      url: "https://meet.google.com/abc-defg-hij",
      is_active: true,
      muted: false,
      participants: [{ name: "Alice", is_self: true }],
    });
  });

  it("rejects invalid origin", () => {
    const message = parseIncomingMessage({
      type: "meeting_state",
      url: "https://example.com/abc",
      is_active: true,
      muted: false,
      participants: [],
    });

    expect(message).toBeNull();
  });

  it("parses meeting_ended payload", () => {
    const message = parseIncomingMessage({
      type: "meeting_ended",
      url: "https://meet.google.com/abc-defg-hij",
      is_active: false,
    });

    expect(message).toEqual({
      type: "meeting_ended",
      url: "https://meet.google.com/abc-defg-hij",
      is_active: false,
    });
  });
});
