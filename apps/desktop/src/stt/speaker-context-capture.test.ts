import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  context: { intervals: [] } as { intervals: Array<Record<string, unknown>> },
  inspect: vi.fn(),
  mic: vi.fn(),
  currentDevice: vi.fn(),
  participants: vi.fn(),
  eventJson: "{}",
}));

vi.mock("@anlg/plugin-detect", () => ({
  commands: {
    inspectMeetingAccessibility: mocks.inspect,
    listMicUsingApplications: mocks.mic,
  },
}));
vi.mock("@anlg/plugin-transcription", () => ({
  commands: { getCurrentMicrophoneDevice: mocks.currentDevice },
}));
vi.mock("~/stt/queries", () => ({
  getSessionParticipantHumanIds: mocks.participants,
}));
vi.mock("~/db/write-queue", () => ({
  enqueueDatabaseWrite: (_key: string, write: () => Promise<void>) => write(),
}));
vi.mock("~/db", () => ({
  liveQueryClient: {
    execute: async (sql: string) =>
      sql.includes("AS context")
        ? [{ context: JSON.stringify(mocks.context) }]
        : [
            {
              title: "John x Alex",
              name: "John",
              event_json: mocks.eventJson,
              owner_user_id: "self",
              aliases: null,
            },
          ],
  },
  executeTransaction: async (writes: Array<{ params: unknown[] }>) => {
    mocks.context = JSON.parse(writes[0]!.params[0] as string);
  },
}));

import {
  observeSpeakerMicrophone,
  startSpeakerContextCapture,
  stopSpeakerContextCapture,
} from "./speaker-context-capture";

const inspection = {
  activeCall: true,
  accessibilityTrusted: true,
  platform: "googleMeet",
  windowTitle: "Meet - abc-defg-hij",
  app: { id: "chrome", name: "Chrome" },
  warnings: [],
};

const latestInterval = () =>
  mocks.context.intervals[mocks.context.intervals.length - 1];

describe("capturing speaker context", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    mocks.context = { intervals: [] };
    mocks.inspect.mockResolvedValue({ status: "ok", data: [inspection] });
    mocks.mic.mockResolvedValue({ status: "ok", data: [inspection.app] });
    mocks.currentDevice.mockResolvedValue({
      status: "ok",
      data: "AirPods Pro",
    });
    mocks.participants.mockResolvedValue([]);
    mocks.eventJson = "{}";
  });
  afterEach(async () => {
    await stopSpeakerContextCapture("session");
    vi.useRealTimers();
  });

  it("records a joined Meet call without any calendar event or attendees", async () => {
    startSpeakerContextCapture("session");
    observeSpeakerMicrophone("session", { isolated: true });
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]).toMatchObject({
      active_call: true,
      calendar_call: false,
      title: "John x Alex",
      mic_isolated: true,
      participants: [],
    });
  });

  it("does not use an open Meet tab or another app's microphone as an active call", async () => {
    mocks.inspect.mockResolvedValue({
      status: "ok",
      data: [{ ...inspection, activeCall: false }],
    });
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]?.active_call).toBe(false);
    mocks.inspect.mockResolvedValue({ status: "ok", data: [inspection] });
    mocks.mic.mockResolvedValue({
      status: "ok",
      data: [{ id: "voice-memo", name: "Voice Memo" }],
    });
    await vi.advanceTimersByTimeAsync(5000);
    expect(
      mocks.context.intervals.every(
        (interval) => interval.active_call === false,
      ),
    ).toBe(true);
  });

  it("keeps the call while its app holds the microphone through failed accessibility reads", async () => {
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    mocks.inspect.mockResolvedValue({ status: "ok", data: [] });
    await vi.advanceTimersByTimeAsync(5000);
    expect(latestInterval()?.active_call).toBe(true);
    mocks.mic.mockRejectedValue(new Error("mic lookup unavailable"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(latestInterval()?.active_call).toBe(true);
    mocks.mic.mockResolvedValue({ status: "ok", data: [] });
    await vi.advanceTimersByTimeAsync(5000);
    expect(latestInterval()?.active_call).toBe(false);
  });

  it("ends the call when the meeting window is positively out of the call", async () => {
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    mocks.inspect.mockResolvedValue({
      status: "ok",
      data: [{ ...inspection, activeCall: false }],
    });
    await vi.advanceTimersByTimeAsync(5000);
    expect(latestInterval()?.active_call).toBe(false);
  });

  it("treats a personal input device as isolated without a runtime verdict", async () => {
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]?.mic_isolated).toBe(true);
  });

  it("counts a browser holding the microphone toward a scheduled Meet link", async () => {
    mocks.eventJson = JSON.stringify({
      meeting_link: "https://meet.google.com/abc-defg-hij",
    });
    mocks.inspect.mockResolvedValue({ status: "ok", data: [] });
    mocks.mic.mockResolvedValue({
      status: "ok",
      data: [{ id: "com.google.Chrome", name: "Google Chrome" }],
    });
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]).toMatchObject({
      active_call: false,
      calendar_call: true,
    });
  });

  it("closes headset evidence when the actual input changes to a room device", async () => {
    startSpeakerContextCapture("session");
    observeSpeakerMicrophone("session", { isolated: true });
    await vi.advanceTimersByTimeAsync(0);
    vi.setSystemTime(3000);
    observeSpeakerMicrophone("session", { device: "Jabra Speak 750" });
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]?.end_ms).toBe(3000);
    expect(mocks.context.intervals[1]).toMatchObject({
      start_ms: 3000,
      mic_isolated: null,
      shared_microphone: true,
    });
    await stopSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(20000);
    expect(
      mocks.context.intervals.every(
        (interval) => Number(interval.end_ms) <= 3000,
      ),
    ).toBe(true);
  });

  it("does not treat headphone output with a built-in input as an isolated microphone", async () => {
    mocks.currentDevice.mockResolvedValue({
      status: "ok",
      data: "MacBook Pro Microphone",
    });
    observeSpeakerMicrophone("session", { isolated: true });
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals[0]?.mic_isolated).toBe(false);
  });

  it("closes evidence without waiting for a hung inspection and does not queue polls", async () => {
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    mocks.inspect.mockClear();
    let finish!: (value: unknown) => void;
    mocks.inspect.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    await vi.advanceTimersByTimeAsync(15000);
    expect(mocks.inspect).toHaveBeenCalledTimes(1);
    await stopSpeakerContextCapture("session");
    expect(mocks.context.intervals[0]?.end_ms).toBeLessThanOrEqual(Date.now());
    finish({ status: "ok", data: [inspection] });
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.context.intervals).toHaveLength(1);
  });

  it("does not write stale inspection results after capture stops", async () => {
    let finish!: (value: unknown) => void;
    mocks.inspect.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    startSpeakerContextCapture("session");
    await vi.advanceTimersByTimeAsync(0);
    const stopping = stopSpeakerContextCapture("session");
    finish({ status: "ok", data: [inspection] });
    await stopping;
    expect(mocks.context.intervals).toEqual([]);
  });
});
