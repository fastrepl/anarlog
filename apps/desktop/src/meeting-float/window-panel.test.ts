import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  show: vi.fn(),
  hide: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@anlg/plugin-windows", () => ({
  commands: {
    floatingBarShow: mocks.show,
    floatingBarHide: mocks.hide,
    floatingBarUpdate: mocks.update,
  },
}));

import type { FloatingRouteState } from "./route-state";
import { createFloatingMeetingWindowSynchronizer } from "./window-panel";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, reject, resolve };
}

const bubbles = [
  {
    id: "bubble-1",
    speakerLabel: "You",
    text: "hello",
    isSelf: true,
    isFinal: true,
    startMs: 0,
    endMs: 100,
    overlapsPrevious: false,
    overlapsNext: false,
  },
];

function routeState(amplitude: number): FloatingRouteState {
  return {
    sessionId: "session-1",
    title: "Meeting",
    amplitude,
    status: "recording",
    colorScheme: "dark",
    opacity: 0.78,
    liveCaptionOpacity: 0.3,
    liveCaptionWidth: 440,
    liveCaptionLineCount: 1,
    liveCaptionPosition: "topCenter",
    liveCaptionMinimized: true,
    liveCaptionToggleVisible: true,
    transcriptBubbles: bubbles,
  };
}

describe("floating meeting window synchronizer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.show.mockResolvedValue({ status: "ok", data: null });
    mocks.hide.mockResolvedValue({ status: "ok", data: null });
    mocks.update.mockResolvedValue({ status: "ok", data: null });
  });

  it("keeps one sync in flight and coalesces updates to the latest state", async () => {
    const firstUpdate = deferred<{ status: "ok"; data: null }>();
    mocks.update.mockImplementationOnce(() => firstUpdate.promise);
    const synchronizer = createFloatingMeetingWindowSynchronizer();

    synchronizer.update(routeState(0.1));
    await vi.waitFor(() => expect(mocks.update).toHaveBeenCalledOnce());

    synchronizer.update(routeState(0.2));
    synchronizer.update(routeState(0.3));
    expect(mocks.update).toHaveBeenCalledOnce();

    firstUpdate.resolve({ status: "ok", data: null });
    await vi.waitFor(() => expect(mocks.update).toHaveBeenCalledTimes(2));
    expect(mocks.update).toHaveBeenLastCalledWith(
      expect.objectContaining({ amplitude: 0.3, transcriptBubbles: null }),
    );
    expect(mocks.update).not.toHaveBeenCalledWith(
      expect.objectContaining({ amplitude: 0.2 }),
    );
    expect(mocks.show).toHaveBeenCalledOnce();

    await synchronizer.dispose();
  });

  it("orders a newer hide after a stale pending update", async () => {
    const pendingUpdate = deferred<{ status: "ok"; data: null }>();
    mocks.update.mockImplementationOnce(() => pendingUpdate.promise);
    const synchronizer = createFloatingMeetingWindowSynchronizer();

    synchronizer.update(routeState(0.1));
    await vi.waitFor(() => expect(mocks.update).toHaveBeenCalledOnce());

    const disposed = synchronizer.dispose();
    expect(mocks.hide).not.toHaveBeenCalled();

    pendingUpdate.resolve({ status: "ok", data: null });
    await disposed;

    expect(mocks.update).toHaveBeenCalledOnce();
    expect(mocks.hide).toHaveBeenCalled();
    expect(mocks.hide.mock.invocationCallOrder[0]).toBeGreaterThan(
      mocks.update.mock.invocationCallOrder[0]!,
    );
  });

  it("still applies a newer hide when the stale update rejects", async () => {
    const pendingUpdate = deferred<{ status: "ok"; data: null }>();
    mocks.update.mockImplementationOnce(() => pendingUpdate.promise);
    const synchronizer = createFloatingMeetingWindowSynchronizer();

    synchronizer.update(routeState(0.1));
    await vi.waitFor(() => expect(mocks.update).toHaveBeenCalledOnce());

    const disposed = synchronizer.dispose();
    pendingUpdate.reject(new Error("IPC disconnected"));
    await disposed;

    expect(mocks.hide).toHaveBeenCalled();
    expect(mocks.hide.mock.invocationCallOrder[0]).toBeGreaterThan(
      mocks.update.mock.invocationCallOrder[0]!,
    );
  });
});
