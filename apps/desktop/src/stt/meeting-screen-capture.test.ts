import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { startMeetingScreenCapture } from "./meeting-screen-capture";

const {
  captureMeetingScreenShareMock,
  checkPermissionMock,
  persistMeetingScreenCaptureMock,
} = vi.hoisted(() => ({
  captureMeetingScreenShareMock: vi.fn(),
  checkPermissionMock: vi.fn(),
  persistMeetingScreenCaptureMock: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: { captureMeetingScreenShare: captureMeetingScreenShareMock },
}));

vi.mock("@anlg/plugin-permissions", () => ({
  commands: { checkPermission: checkPermissionMock },
}));

vi.mock("~/stt/meeting-screen-records", () => ({
  persistMeetingScreenCapture: persistMeetingScreenCaptureMock,
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { warning: vi.fn() },
}));

const frame = {
  sharing: true,
  app: { id: "us.zoom.xos", name: "Zoom" },
  platform: "zoom" as const,
  jpeg: [0xff, 0xd8],
  width: 1920,
  height: 1080,
};

describe("startMeetingScreenCapture", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    checkPermissionMock.mockResolvedValue({ status: "ok", data: "authorized" });
    persistMeetingScreenCaptureMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("resets the sampler only for a new capture and persists only new frames", async () => {
    captureMeetingScreenShareMock
      .mockResolvedValueOnce({ status: "ok", data: { ...frame, jpeg: null } })
      .mockResolvedValueOnce({ status: "ok", data: frame })
      .mockResolvedValue({ status: "ok", data: { ...frame, jpeg: null } });

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(3_000);
    await vi.advanceTimersByTimeAsync(3_000);
    await stop();

    expect(
      captureMeetingScreenShareMock.mock.calls.map(([reset]) => reset),
    ).toEqual([true, false, false]);
    expect(persistMeetingScreenCaptureMock).toHaveBeenCalledTimes(1);
    expect(persistMeetingScreenCaptureMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      capture: frame,
    });
  });

  test("never captures the screen when Screen Recording is denied", async () => {
    checkPermissionMock.mockResolvedValue({ status: "ok", data: "denied" });

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(3_000);
    await stop();

    expect(captureMeetingScreenShareMock).not.toHaveBeenCalled();
  });
});
