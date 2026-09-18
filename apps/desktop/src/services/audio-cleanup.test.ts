import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  save: vi.fn(),
  clear: vi.fn(),
  error: vi.fn(),
  dismiss: vi.fn(),
  listen: vi.fn(),
  snapshot: vi.fn(),
  acknowledge: vi.fn(),
}));
vi.mock("@anlg/plugin-transcription", () => ({
  commands: {
    getCaptureAudioCleanupStatus: mocks.snapshot,
    acknowledgeCaptureAudioCleanupStatus: mocks.acknowledge,
  },
  events: { captureStatusEvent: { listen: mocks.listen } },
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { error: mocks.error, dismiss: mocks.dismiss },
}));
vi.mock("~/stt/capture-result", () => ({
  saveIncompleteCapture: mocks.save,
  clearCaptureAudioDeletionFailure: mocks.clear,
}));

import {
  handleCaptureCleanupStatus,
  listenForCaptureCleanup,
} from "./audio-cleanup";

beforeEach(() => {
  vi.resetAllMocks();
  mocks.acknowledge.mockResolvedValue({ status: "ok", data: null });
});

it("surfaces and persists cleanup failures without an active recording", async () => {
  await handleCaptureCleanupStatus({
    type: "audio_error",
    session_id: "old-session",
    error: "audio_deletion_failed: denied",
    device: null,
    is_fatal: false,
  });
  expect(mocks.error).toHaveBeenCalledWith(
    "Audio could not be deleted",
    expect.objectContaining({
      id: "audio-deletion-old-session",
      duration: Infinity,
    }),
  );
  expect(mocks.save).toHaveBeenCalledWith(
    "old-session",
    "audio-cleanup",
    false,
    true,
  );
});

it("clears the durable deletion failure only on confirmed cleanup", async () => {
  await handleCaptureCleanupStatus({
    type: "audio_error",
    session_id: "old-session",
    error: "audio_deletion_completed",
    device: null,
    is_fatal: false,
  });
  expect(mocks.clear).toHaveBeenCalledWith("old-session");
  expect(mocks.dismiss).toHaveBeenCalledWith("audio-deletion-old-session");
});

it("ignores ordinary recording errors", async () => {
  await handleCaptureCleanupStatus({
    type: "audio_error",
    session_id: "session",
    error: "audio_storage_backpressure",
    device: null,
    is_fatal: false,
  });
  expect(mocks.save).not.toHaveBeenCalled();
  expect(mocks.clear).not.toHaveBeenCalled();
  expect(mocks.error).not.toHaveBeenCalled();
});

it("restores a startup failure from native state after subscribing", async () => {
  const stop = vi.fn();
  mocks.listen.mockResolvedValue(stop);
  mocks.snapshot.mockResolvedValue({
    status: "ok",
    data: { "old-session": "audio_deletion_failed: denied" },
  });
  expect(await listenForCaptureCleanup()).toBe(stop);
  expect(mocks.save).toHaveBeenCalledWith(
    "old-session",
    "audio-cleanup",
    false,
    true,
  );
});

it("does not let an older startup snapshot overwrite a newer cleanup event", async () => {
  let listener!: (event: { payload: unknown }) => void;
  mocks.listen.mockImplementation(async (callback) => {
    listener = callback;
    return vi.fn();
  });
  mocks.snapshot.mockImplementation(async () => {
    listener({
      payload: {
        type: "audio_error",
        session_id: "old-session",
        error: "audio_deletion_completed",
        is_fatal: false,
        device: null,
      },
    });
    return {
      status: "ok",
      data: { "old-session": "audio_deletion_failed: denied" },
    };
  });
  await listenForCaptureCleanup();
  expect(mocks.clear).toHaveBeenCalledWith("old-session");
  expect(mocks.save).not.toHaveBeenCalled();
});

it("only acknowledges cleanup after persistence succeeds", async () => {
  const payload = {
    type: "audio_error" as const,
    session_id: "session",
    error: "audio_deletion_failed: denied",
    device: null,
    is_fatal: false,
  };
  mocks.save.mockRejectedValueOnce(new Error("database full"));
  await expect(handleCaptureCleanupStatus(payload)).rejects.toThrow(
    "database full",
  );
  expect(mocks.acknowledge).not.toHaveBeenCalled();
  await handleCaptureCleanupStatus(payload);
  expect(mocks.acknowledge).toHaveBeenCalledWith(
    "session",
    "audio_deletion_failed: denied",
  );
});

it("continues restoring other sessions when one persistence write fails", async () => {
  vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.listen.mockResolvedValue(vi.fn());
  mocks.snapshot.mockResolvedValue({
    status: "ok",
    data: {
      broken: "audio_deletion_failed: denied",
      failed: "audio_deletion_failed: denied",
      completed: "audio_deletion_completed",
    },
  });
  mocks.save.mockRejectedValueOnce(new Error("database full"));
  await listenForCaptureCleanup();
  expect(mocks.save).toHaveBeenCalledWith(
    "failed",
    "audio-cleanup",
    false,
    true,
  );
  expect(mocks.clear).toHaveBeenCalledWith("completed");
  expect(mocks.acknowledge).not.toHaveBeenCalledWith(
    "broken",
    "audio_deletion_failed: denied",
  );
  expect(mocks.acknowledge).toHaveBeenCalledWith(
    "failed",
    "audio_deletion_failed: denied",
  );
  expect(mocks.acknowledge).toHaveBeenCalledWith(
    "completed",
    "audio_deletion_completed",
  );
  vi.restoreAllMocks();
});

it("handles subscription rejection and returns a safe unsubscriber", async () => {
  const log = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.listen.mockRejectedValue(new Error("event service unavailable"));
  mocks.snapshot.mockResolvedValue({ status: "ok", data: {} });
  const stop = await listenForCaptureCleanup();
  expect(() => stop()).not.toThrow();
  expect(log).toHaveBeenCalledWith(
    "[audio-cleanup] failed to subscribe to cleanup status",
    expect.any(Error),
  );
  vi.restoreAllMocks();
});

it("uses the generic notification ID only for startup-wide failures", async () => {
  await handleCaptureCleanupStatus({
    type: "audio_error",
    session_id: "",
    error: "audio_deletion_failed: unavailable",
    device: null,
    is_fatal: false,
  });
  expect(mocks.error).toHaveBeenCalledWith(
    "Audio cleanup could not finish",
    expect.objectContaining({ id: "audio-cleanup" }),
  );
  expect(mocks.save).not.toHaveBeenCalled();
});

it("keeps completion behind an in-flight failure write", async () => {
  let listener!: (event: { payload: unknown }) => void;
  let finishSave!: () => void;
  const saving = new Promise<void>((resolve) => {
    finishSave = resolve;
  });
  mocks.save.mockReturnValue(saving);
  mocks.listen.mockImplementation(async (callback) => {
    listener = callback;
    return vi.fn();
  });
  mocks.snapshot.mockResolvedValue({ status: "ok", data: {} });
  await listenForCaptureCleanup();
  const event = {
    type: "audio_error",
    session_id: "session",
    is_fatal: false,
    device: null,
  };
  listener({ payload: { ...event, error: "audio_deletion_failed: denied" } });
  await vi.waitFor(() => expect(mocks.save).toHaveBeenCalled());
  listener({ payload: { ...event, error: "audio_deletion_completed" } });
  expect(mocks.clear).not.toHaveBeenCalled();
  finishSave();
  await vi.waitFor(() =>
    expect(mocks.acknowledge).toHaveBeenCalledWith(
      "session",
      "audio_deletion_completed",
    ),
  );
  expect(mocks.clear).toHaveBeenCalledWith("session");
});

it("replays recovery failures without marking retained audio for deletion", async () => {
  mocks.listen.mockResolvedValue(vi.fn());
  mocks.snapshot.mockResolvedValue({
    status: "ok",
    data: { session: "audio_recovery_failed: rename denied" },
  });
  await listenForCaptureCleanup();
  expect(mocks.save).toHaveBeenCalledWith("session", "audio-recovery", false);
  expect(mocks.error).toHaveBeenCalledWith(
    "Audio could not be recovered",
    expect.objectContaining({ id: "audio-recovery-session" }),
  );
  expect(mocks.acknowledge).toHaveBeenCalledWith(
    "session",
    "audio_recovery_failed: rename denied",
  );
  await handleCaptureCleanupStatus({
    type: "audio_error",
    session_id: "session",
    error: "audio_recovery_completed",
    device: null,
    is_fatal: false,
  });
  expect(mocks.clear).not.toHaveBeenCalled();
  expect(mocks.dismiss).toHaveBeenCalledWith("audio-recovery-session");
});
