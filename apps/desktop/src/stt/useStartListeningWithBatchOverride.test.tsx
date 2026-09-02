import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  getSessionMode: vi.fn(),
  startListening: vi.fn(),
  stopTranscription: vi.fn(),
}));

vi.mock("./contexts", () => ({
  useListener: (selector: (state: unknown) => unknown) =>
    selector({
      getSessionMode: mocks.getSessionMode,
      stopTranscription: mocks.stopTranscription,
    }),
}));

vi.mock("./useStartListening", () => ({
  useStartListening: () => mocks.startListening,
}));

import { useStartListeningWithBatchOverride } from "./useStartListeningWithBatchOverride";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.getSessionMode.mockReturnValue("inactive");
  mocks.stopTranscription.mockResolvedValue(undefined);
  mocks.startListening.mockResolvedValue(undefined);
});

test("starts listening without touching inactive transcription", async () => {
  const { result } = renderHook(() =>
    useStartListeningWithBatchOverride("session-1"),
  );

  await act(result.current);

  expect(mocks.stopTranscription).not.toHaveBeenCalled();
  expect(mocks.startListening).toHaveBeenCalledTimes(1);
});

test("stops batch transcription before resuming listening", async () => {
  mocks.getSessionMode.mockReturnValue("running_batch");
  const { result } = renderHook(() =>
    useStartListeningWithBatchOverride("session-1"),
  );

  await act(result.current);

  expect(mocks.stopTranscription).toHaveBeenCalledWith("session-1");
  expect(mocks.startListening).toHaveBeenCalledTimes(1);
  expect(mocks.stopTranscription).toHaveBeenCalledBefore(mocks.startListening);
});
