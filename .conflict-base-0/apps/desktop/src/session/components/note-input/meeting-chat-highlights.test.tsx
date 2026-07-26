import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { MeetingChatHighlights } from "./meeting-chat-highlights";

const { openUrlMock, useMeetingChatRecordsMock } = vi.hoisted(() => ({
  openUrlMock: vi.fn(),
  useMeetingChatRecordsMock: vi.fn(),
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openUrl: openUrlMock },
}));

vi.mock("~/stt/meeting-chat-records", async (importOriginal) => ({
  ...(await importOriginal<typeof import("~/stt/meeting-chat-records")>()),
  useMeetingChatRecords: useMeetingChatRecordsMock,
}));

describe("MeetingChatHighlights", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useMeetingChatRecordsMock.mockReturnValue([]);
  });

  test("stays hidden until a meeting-chat record exists", () => {
    const { container } = render(
      <MeetingChatHighlights sessionId="session-1" />,
    );

    expect(container.innerHTML).toBe("");
  });

  test("renders chronological metadata and opens captured links externally", () => {
    useMeetingChatRecordsMock.mockReturnValue([
      {
        id: "msg-1",
        platform: "zoom",
        surface: "native",
        sender: "Ada",
        timestamp: "10:42 AM",
        direction: "incoming",
        text: "Review https://example.com/spec",
        links: ["https://example.com/spec"],
        capturedAt: "2026-07-13T10:00:00.000Z",
      },
    ]);

    render(<MeetingChatHighlights sessionId="session-1" />);

    expect(screen.getByText("Zoom · 10:42 AM · Ada · received")).not.toBeNull();
    fireEvent.click(
      screen.getByRole("link", { name: "https://example.com/spec" }),
    );
    expect(openUrlMock).toHaveBeenCalledWith("https://example.com/spec", null);
  });

  test.each([
    ["zoom", "Zoom"],
    ["googleMeet", "Google Meet"],
    ["microsoftTeams", "Microsoft Teams"],
    ["slack", "Slack"],
    ["discord", "Discord"],
    ["webex", "Webex"],
    ["unknown", "Meeting app"],
  ] as const)("labels %s records as %s", (platform, label) => {
    useMeetingChatRecordsMock.mockReturnValue([
      {
        id: `msg-${platform}`,
        platform,
        surface: platform === "unknown" ? "unknown" : "web",
        sender: null,
        timestamp: null,
        direction: null,
        text: "Agenda item",
        links: [],
        capturedAt: "2026-07-13T10:00:00.000Z",
      },
    ]);

    render(<MeetingChatHighlights sessionId="session-1" />);

    expect(screen.getByText(label)).not.toBeNull();
  });
});
