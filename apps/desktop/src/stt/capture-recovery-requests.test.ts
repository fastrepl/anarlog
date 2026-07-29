import { beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  emit: vi.fn(),
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  emit: mocks.emit,
  listen: mocks.listen,
}));

import {
  listenCaptureRecoveryRequests,
  requestCaptureRecovery,
} from "./capture-recovery-requests";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.emit.mockResolvedValue(undefined);
  mocks.listen.mockResolvedValue(vi.fn());
});

test("broadcasts a recovery wakeup to the main window", async () => {
  await requestCaptureRecovery("session-1");

  expect(mocks.emit).toHaveBeenCalledWith("anlg:capture-recovery-request", {
    sessionId: "session-1",
  });
});

test("accepts only non-empty session identifiers", async () => {
  const onRequest = vi.fn();
  await listenCaptureRecoveryRequests(onRequest);
  const handler = mocks.listen.mock.calls[0]?.[1];

  handler?.({ payload: null });
  handler?.({ payload: { sessionId: "" } });
  handler?.({ payload: { sessionId: 42 } });
  handler?.({ payload: { sessionId: "session-1" } });

  expect(onRequest).toHaveBeenCalledOnce();
  expect(onRequest).toHaveBeenCalledWith("session-1");
});
