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

vi.mock("~/stt/meeting-chat-records", () => ({
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
});
