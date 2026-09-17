import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  save: vi.fn(),
  clear: vi.fn(),
  error: vi.fn(),
  dismiss: vi.fn(),
  listen: vi.fn(),
  snapshot: vi.fn(),
}));
vi.mock("@anlg/plugin-transcription", () => ({
  commands: { getCaptureAudioCleanupStatus: mocks.snapshot },
  events: { captureStatusEvent: { listen: mocks.listen } },
}));
vi.mock("@anlg/ui/components/ui/toast", () => ({
  sonnerToast: { error: mocks.error, dismiss: mocks.dismiss },
}));
vi.mock("~/stt/capture-result", () => ({
  saveIncompleteCapture: mocks.save,
  clearCaptureAudioDeletionFailure: mocks.clear,
}));

import {
  handleCaptureCleanupStatus,
  listenForCaptureCleanup,
} from "./audio-cleanup";

beforeEach(() => vi.clearAllMocks());

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
    expect.objectContaining({ duration: Infinity }),
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
    data: { "old-session": true },
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
    return { status: "ok", data: { "old-session": true } };
  });
  await listenForCaptureCleanup();
  expect(mocks.clear).toHaveBeenCalledWith("old-session");
  expect(mocks.save).not.toHaveBeenCalled();
});
