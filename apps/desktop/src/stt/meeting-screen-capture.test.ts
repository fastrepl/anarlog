import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import { startMeetingScreenCapture } from "./meeting-screen-capture";

const {
  captureMeetingScreenShareMock,
  checkPermissionMock,
  persistMeetingScreenCaptureMock,
  platformMock,
} = vi.hoisted(() => ({
  captureMeetingScreenShareMock: vi.fn(),
  checkPermissionMock: vi.fn(),
  persistMeetingScreenCaptureMock: vi.fn(),
  platformMock: vi.fn(),
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: { captureMeetingScreenShare: captureMeetingScreenShareMock },
}));

vi.mock("@anlg/plugin-permissions", () => ({
  commands: { checkPermission: checkPermissionMock },
}));

vi.mock("@tauri-apps/plugin-os", () => ({ platform: platformMock }));

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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe("startMeetingScreenCapture", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    platformMock.mockReturnValue("macos");
    checkPermissionMock.mockResolvedValue({ status: "ok", data: "authorized" });
    persistMeetingScreenCaptureMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test("persists only frames the sampler kept", async () => {
    captureMeetingScreenShareMock
      .mockResolvedValueOnce({ status: "ok", data: { ...frame, jpeg: null } })
      .mockResolvedValueOnce({ status: "ok", data: frame })
      .mockResolvedValue({ status: "ok", data: { ...frame, jpeg: null } });

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(6_000);
    await stop();

    expect(persistMeetingScreenCaptureMock).toHaveBeenCalledTimes(1);
    expect(persistMeetingScreenCaptureMock).toHaveBeenCalledWith({
      sessionId: "session-1",
      capture: frame,
    });
  });

  test("never captures on macOS when Screen Recording is denied", async () => {
    checkPermissionMock.mockResolvedValue({ status: "ok", data: "denied" });

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(3_000);
    await stop();

    expect(captureMeetingScreenShareMock).not.toHaveBeenCalled();
  });

  test("captures on Windows, where there is no Screen Recording permission", async () => {
    platformMock.mockReturnValue("windows");
    checkPermissionMock.mockResolvedValue({ status: "ok", data: "denied" });
    captureMeetingScreenShareMock.mockResolvedValue({
      status: "ok",
      data: frame,
    });

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    await stop();

    expect(persistMeetingScreenCaptureMock).toHaveBeenCalledTimes(1);
  });

  test("drops a frame that finishes after recording stopped", async () => {
    const pending = deferred<unknown>();
    captureMeetingScreenShareMock.mockReturnValue(pending.promise);

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => true,
    });
    await vi.advanceTimersByTimeAsync(0);
    let stopped = false;
    const stopping = stop().then(() => {
      stopped = true;
    });
    await vi.advanceTimersByTimeAsync(0);
    expect(stopped).toBe(false);

    pending.resolve({ status: "ok", data: frame });
    await stopping;

    expect(persistMeetingScreenCaptureMock).not.toHaveBeenCalled();
  });

  test("drops a frame when the setting is turned off mid-capture", async () => {
    let enabled = true;
    const pending = deferred<unknown>();
    captureMeetingScreenShareMock.mockReturnValue(pending.promise);

    const stop = startMeetingScreenCapture({
      sessionId: "session-1",
      isEnabled: () => enabled,
    });
    await vi.advanceTimersByTimeAsync(0);
    enabled = false;
    pending.resolve({ status: "ok", data: frame });
    await vi.advanceTimersByTimeAsync(0);
    await stop();

    expect(persistMeetingScreenCaptureMock).not.toHaveBeenCalled();
  });
});
