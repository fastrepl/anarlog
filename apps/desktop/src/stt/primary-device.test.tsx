import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => {
  const listeners = new Set<() => void>();
  const live = { sessionId: "session-1" as string | null, status: "active" };
  return {
    listeners,
    live,
    stop: vi.fn(),
    toast: Object.assign(vi.fn(), { info: vi.fn(), dismiss: vi.fn() }),
    requestMeetingDevices: vi.fn(),
    getSession: vi.fn(() =>
      Promise.resolve({
        data: {
          session: { access_token: "token", user: { is_anonymous: false } },
        },
      }),
    ),
    preferredFingerprint: "",
  };
});

vi.mock("@anlg/ui/components/ui/toast", () => ({ toast: mocks.toast }));
vi.mock("~/auth/client", () => ({
  supabase: { auth: { getSession: mocks.getSession } },
}));
vi.mock("~/auth/cloudsync-credentials", () => ({
  getDeviceIdentity: () => Promise.resolve({ fingerprint: "this-device" }),
}));
vi.mock("~/auth/sync-devices", () => ({
  requestMeetingDevices: mocks.requestMeetingDevices,
}));
vi.mock("~/settings/queries", () => ({
  getStoredSettingValues: async () => ({
    values: { primary_recording_device: mocks.preferredFingerprint },
    hasValues: new Set(["primary_recording_device"]),
  }),
}));
vi.mock("~/store/zustand/listener/instance", () => ({
  listenerStore: {
    getState: () => ({ live: mocks.live, stop: mocks.stop }),
    subscribe: (listener: () => void) => {
      mocks.listeners.add(listener);
      return () => mocks.listeners.delete(listener);
    },
  },
}));

import {
  consumePrimaryDeviceYield,
  decidePrimaryDevice,
  meetingKeyForEvent,
  PRIMARY_DEVICE_HEARTBEAT_MS,
  startPrimaryDeviceCoordination,
} from "./primary-device";

const event = { tracking_id: "event-1", started_at: "2026-09-28T15:00:00Z" };
const self = { deviceFingerprint: "this-device", deviceName: "Work Mac" };
const other = { deviceFingerprint: "other-device", deviceName: "Home Mac" };

function endRecording() {
  mocks.live.status = "inactive";
  for (const listener of [...mocks.listeners]) listener();
}

async function flush(calls = 1) {
  await vi.waitFor(() =>
    expect(
      mocks.requestMeetingDevices.mock.calls.length,
    ).toBeGreaterThanOrEqual(calls),
  );
  for (let i = 0; i < 10; i += 1) {
    await vi.advanceTimersByTimeAsync(0);
  }
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mocks.toast.mockReset();
  mocks.live.sessionId = "session-1";
  mocks.live.status = "active";
  mocks.preferredFingerprint = "";
});

afterEach(() => {
  cleanup();
  endRecording();
  consumePrimaryDeviceYield("session-1");
  vi.useRealTimers();
});

test("decides from the devices present for the meeting", () => {
  expect(
    decidePrimaryDevice([{ ...self, primary: false }], "this-device"),
  ).toBe("alone");
  expect(
    decidePrimaryDevice(
      [
        { ...self, primary: false },
        { ...other, primary: false },
      ],
      "this-device",
    ),
  ).toBe("ask");
  expect(
    decidePrimaryDevice(
      [
        { ...self, primary: true },
        { ...other, primary: false },
      ],
      "this-device",
    ),
  ).toBe("primary");
  expect(
    decidePrimaryDevice(
      [
        { ...self, primary: false },
        { ...other, primary: true },
      ],
      "this-device",
    ),
  ).toBe("yield");
  expect(
    decidePrimaryDevice(
      [{ ...self, primary: false }],
      "this-device",
      "other-device",
    ),
  ).toBe("alone");
  expect(
    decidePrimaryDevice(
      [
        { ...self, primary: false },
        { ...other, primary: false },
      ],
      "this-device",
      "unavailable-device",
    ),
  ).toBe("ask");
});

test.each([true, false])(
  "a server-confirmed preferred recorder stops a duplicate (automatic=%s)",
  async (automatic) => {
    mocks.preferredFingerprint = "other-device";
    mocks.requestMeetingDevices.mockResolvedValue([
      { ...self, primary: false },
      { ...other, primary: true },
    ]);
    startPrimaryDeviceCoordination({
      sessionId: "session-1",
      event,
      automatic,
    });
    await flush();

    expect(mocks.stop).toHaveBeenCalled();
    expect(consumePrimaryDeviceYield("session-1")).toBe(true);
    expect(mocks.toast).not.toHaveBeenCalled();
    expect(mocks.toast.info).toHaveBeenCalledWith(
      "Recording on Home Mac",
      expect.anything(),
    );
    expect(
      mocks.requestMeetingDevices.mock.calls.map(([request]) => request.intent),
    ).not.toContain("claim");
  },
);

test("a preferred recorder claims an unclaimed meeting and respects a later shared winner", async () => {
  mocks.preferredFingerprint = "this-device";
  let primaryFingerprint = "";
  mocks.requestMeetingDevices.mockImplementation(async ({ intent }) => {
    if (intent === "claim") primaryFingerprint = "this-device";
    return [
      { ...self, primary: primaryFingerprint === "this-device" },
      { ...other, primary: primaryFingerprint === "other-device" },
    ];
  });
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush(2);
  expect(primaryFingerprint).toBe("this-device");

  primaryFingerprint = "other-device";
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(primaryFingerprint).toBe("other-device");
  expect(consumePrimaryDeviceYield("session-1")).toBe(true);
  expect(mocks.stop).toHaveBeenCalled();
});

test("a secondary manual start still checks the saved primary after a network failure", async () => {
  mocks.preferredFingerprint = "other-device";
  mocks.requestMeetingDevices
    .mockRejectedValueOnce(new Error("offline"))
    .mockResolvedValue([
      { ...self, primary: false },
      { ...other, primary: true },
    ]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: false,
  });
  await flush();
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);

  expect(mocks.stop).toHaveBeenCalled();
  expect(consumePrimaryDeviceYield("session-1")).toBe(true);
  expect(
    mocks.requestMeetingDevices.mock.calls.map(([request]) => request.intent),
  ).not.toContain("claim");
});

test("a saved primary that is not recording does not stop a lone secondary device", async () => {
  mocks.preferredFingerprint = "other-device";
  mocks.requestMeetingDevices.mockResolvedValue([{ ...self, primary: false }]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.stop).not.toHaveBeenCalled();
  expect(mocks.toast).not.toHaveBeenCalled();

  mocks.requestMeetingDevices.mockResolvedValue([
    { ...self, primary: false },
    { ...other, primary: true },
  ]);
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.stop).toHaveBeenCalled();
  expect(consumePrimaryDeviceYield("session-1")).toBe(true);
});

test("derives the same opaque key for the same calendar event", async () => {
  const key = await meetingKeyForEvent(event);
  expect(key).toMatch(/^[0-9a-f]{64}$/);
  expect(await meetingKeyForEvent({ ...event })).toBe(key);
  expect(await meetingKeyForEvent({ ...event, started_at: "other" })).not.toBe(
    key,
  );
  expect(await meetingKeyForEvent({ tracking_id: "", started_at: "x" })).toBe(
    null,
  );
});

test("manual starts claim the meeting", async () => {
  mocks.requestMeetingDevices.mockResolvedValue([{ ...self, primary: true }]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: false,
  });
  await flush();

  expect(mocks.requestMeetingDevices).toHaveBeenCalledWith(
    expect.objectContaining({ intent: "claim", fingerprint: "this-device" }),
  );
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
    expect.objectContaining({ intent: "present" }),
  );
});

test("asks when another device is recording, and interaction claims", async () => {
  mocks.requestMeetingDevices.mockResolvedValue([
    { ...self, primary: false },
    { ...other, primary: false },
  ]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();

  expect(mocks.requestMeetingDevices).toHaveBeenCalledWith(
    expect.objectContaining({ intent: "present" }),
  );
  expect(mocks.toast).toHaveBeenCalledWith(
    expect.anything(),
    expect.objectContaining({ id: "primary-device:session-1" }),
  );

  mocks.requestMeetingDevices.mockResolvedValue([
    { ...self, primary: true },
    { ...other, primary: false },
  ]);
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "a" }));
  await flush(2);

  expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
    expect.objectContaining({ intent: "claim" }),
  );
  expect(mocks.toast.dismiss).toHaveBeenCalledWith("primary-device:session-1");
  expect(mocks.stop).not.toHaveBeenCalled();
});

test.each(["Yes", "No"])(
  "clicking the recording question keeps it unanswered until %s is selected",
  async (answer) => {
    const { toast, Toaster } = await vi.importActual<
      typeof import("@anlg/ui/components/ui/toast")
    >("@anlg/ui/components/ui/toast");
    mocks.toast.mockImplementation(toast);
    render(<Toaster />);
    mocks.requestMeetingDevices.mockResolvedValue([
      { ...self, primary: false },
      { ...other, primary: false },
    ]);
    startPrimaryDeviceCoordination({
      sessionId: "session-1",
      event,
      automatic: true,
    });
    await flush();

    try {
      fireEvent.pointerDown(
        screen.getByText(String(mocks.toast.mock.calls[0][0])),
      );
      fireEvent.click(screen.getByText(String(mocks.toast.mock.calls[0][0])));
      expect(screen.queryByRole("button", { name: "Dismiss" })).toBeNull();
      await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
      expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
        expect.objectContaining({ intent: "present" }),
      );
      expect(
        mocks.requestMeetingDevices.mock.calls.map(
          ([request]) => request.intent,
        ),
      ).not.toContain("claim");
      expect(screen.getByRole("status")).toBeTruthy();

      mocks.requestMeetingDevices.mockResolvedValue([
        { ...self, primary: true },
        { ...other, primary: false },
      ]);
      fireEvent.pointerDown(screen.getByRole("button", { name: answer }));
      fireEvent.click(screen.getByRole("button", { name: answer }));
      await flush(3);
      expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
        expect.objectContaining({
          intent: answer === "Yes" ? "claim" : "release",
        }),
      );
      expect(consumePrimaryDeviceYield("session-1")).toBe(answer === "No");
      if (answer === "No") {
        expect(mocks.stop).toHaveBeenCalled();
      } else {
        expect(mocks.stop).not.toHaveBeenCalled();
      }
    } finally {
      toast.dismiss();
      await vi.advanceTimersByTimeAsync(300);
    }
  },
);

test("stops and marks the capture for discard when another device is primary", async () => {
  mocks.requestMeetingDevices.mockResolvedValue([
    { ...self, primary: false },
    { ...other, primary: true },
  ]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();

  expect(mocks.stop).toHaveBeenCalledOnce();
  expect(mocks.toast.info).toHaveBeenCalledWith(
    "Recording on Home Mac",
    expect.anything(),
  );
  expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
    expect.objectContaining({ intent: "release" }),
  );
  expect(consumePrimaryDeviceYield("session-1")).toBe(true);
  expect(consumePrimaryDeviceYield("session-1")).toBe(false);
});

test("keeps recording without coordination when the service is unavailable", async () => {
  mocks.requestMeetingDevices.mockResolvedValue(null);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS * 2);

  expect(mocks.requestMeetingDevices).toHaveBeenCalledOnce();
  expect(mocks.stop).not.toHaveBeenCalled();
});

test("retries after a network failure and releases when recording stops", async () => {
  mocks.requestMeetingDevices.mockRejectedValueOnce(new Error("offline"));
  mocks.requestMeetingDevices.mockResolvedValue([{ ...self, primary: false }]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.requestMeetingDevices).toHaveBeenCalledTimes(2);

  endRecording();
  await flush(3);
  expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
    expect.objectContaining({ intent: "release" }),
  );
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS * 2);
  expect(mocks.requestMeetingDevices).toHaveBeenCalledTimes(3);
});

test("a claim made during an in-flight heartbeat is sent before yielding", async () => {
  let respond: (devices: unknown) => void = () => {};
  mocks.requestMeetingDevices
    .mockResolvedValueOnce([
      { ...self, primary: false },
      { ...other, primary: false },
    ])
    .mockImplementationOnce(() => new Promise((resolve) => (respond = resolve)))
    .mockResolvedValue([
      { ...self, primary: true },
      { ...other, primary: false },
    ]);
  startPrimaryDeviceCoordination({
    sessionId: "session-1",
    event,
    automatic: true,
  });
  await flush();
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.requestMeetingDevices).toHaveBeenCalledTimes(2);

  window.dispatchEvent(new KeyboardEvent("keydown", { key: "a" }));
  respond([
    { ...self, primary: false },
    { ...other, primary: true },
  ]);
  await flush(3);

  expect(mocks.requestMeetingDevices).toHaveBeenLastCalledWith(
    expect.objectContaining({ intent: "claim" }),
  );
  await vi.advanceTimersByTimeAsync(PRIMARY_DEVICE_HEARTBEAT_MS);
  expect(mocks.stop).not.toHaveBeenCalled();
});
