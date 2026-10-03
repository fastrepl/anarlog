import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  toastWarning: vi.fn(),
  toastDismiss: vi.fn(),
  useLiveQuery: vi.fn(),
  sessionMode: "inactive",
  startListening: vi.fn(() => Promise.resolve()),
  requestCaptureRecovery: vi.fn(() => Promise.resolve()),
  requestMainListenerControl: vi.fn(() => Promise.resolve()),
  isMainWebviewWindow: vi.fn(() => true),
}));

vi.mock("@anlg/ui/components/ui/toast", () => ({
  toast: { warning: mocks.toastWarning, dismiss: mocks.toastDismiss },
}));
vi.mock("~/db", () => ({ useLiveQuery: mocks.useLiveQuery }));
vi.mock("./contexts", () => ({
  useListener: (
    selector: (state: { getSessionMode: () => string }) => unknown,
  ) => selector({ getSessionMode: () => mocks.sessionMode }),
}));
vi.mock("./useStartListening", () => ({
  useStartListening: () => mocks.startListening,
}));
vi.mock("./capture-recovery-requests", () => ({
  requestCaptureRecovery: mocks.requestCaptureRecovery,
}));
vi.mock("./window-control", () => ({
  isMainWebviewWindow: mocks.isMainWebviewWindow,
  requestMainListenerControl: mocks.requestMainListenerControl,
}));

import { SavedCaptureAudioPrompt } from "./saved-capture-audio";

type ToastOptions = {
  id: string;
  description: React.ReactElement;
  action: { label: string; onClick: () => void };
  onDismiss: () => void;
};

afterEach(cleanup);

beforeEach(() => {
  vi.clearAllMocks();
  mocks.sessionMode = "inactive";
  mocks.isMainWebviewWindow.mockReturnValue(true);
  mocks.useLiveQuery.mockReturnValue({ data: SAVED_AT });
  localStorage.clear();
});

const SAVED_AT = "2026-10-01T00:00:00.000Z";

function shownToast() {
  expect(mocks.toastWarning).toHaveBeenCalledOnce();
  return mocks.toastWarning.mock.calls[0]![1] as ToastOptions;
}

test("offers to create the meeting note from saved audio", () => {
  render(<SavedCaptureAudioPrompt sessionId="session-1" />);

  const options = shownToast();
  expect(options.id).toBe("capture-audio-saved-session-1");
  expect(options.action.label).toBe("Create meeting note");
  options.action.onClick();
  expect(mocks.requestCaptureRecovery).toHaveBeenCalledWith("session-1");
  expect(mocks.startListening).not.toHaveBeenCalled();
});

test("resumes listening in the same note", () => {
  render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  const { getByRole } = render(shownToast().description);

  getByRole("button", { name: "Resume listening" }).click();

  expect(mocks.startListening).toHaveBeenCalledOnce();
  expect(mocks.requestCaptureRecovery).not.toHaveBeenCalled();
  expect(mocks.toastDismiss).not.toHaveBeenCalled();
});

test("routes resume listening to the main window from note windows", () => {
  mocks.isMainWebviewWindow.mockReturnValue(false);
  render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  const { getByRole } = render(shownToast().description);

  getByRole("button", { name: "Resume listening" }).click();

  expect(mocks.requestMainListenerControl).toHaveBeenCalledWith(
    "start",
    "session-1",
  );
  expect(mocks.startListening).not.toHaveBeenCalled();
});

test("stays hidden without saved audio or while the note is live", () => {
  mocks.useLiveQuery.mockReturnValue({ data: null });
  const { unmount } = render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  unmount();
  mocks.useLiveQuery.mockReturnValue({ data: SAVED_AT });
  mocks.sessionMode = "active";
  render(<SavedCaptureAudioPrompt sessionId="session-1" />);

  expect(mocks.toastWarning).not.toHaveBeenCalled();
});

test("dismisses the toast when the note closes", () => {
  const { unmount } = render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  unmount();
  expect(mocks.toastDismiss).toHaveBeenCalledWith(
    "capture-audio-saved-session-1",
  );
});

test("does not reshow a closed prompt until audio is saved again", () => {
  const first = render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  shownToast().onDismiss();
  first.unmount();
  mocks.toastWarning.mockClear();

  const reopened = render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  expect(mocks.toastWarning).not.toHaveBeenCalled();
  reopened.unmount();

  mocks.useLiveQuery.mockReturnValue({ data: "2026-10-02T00:00:00.000Z" });
  render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  expect(mocks.toastWarning).toHaveBeenCalledOnce();
});

test("does not treat creating the meeting note as closing the prompt", async () => {
  const consoleError = vi
    .spyOn(console, "error")
    .mockImplementation(() => undefined);
  mocks.requestCaptureRecovery.mockRejectedValueOnce(
    new Error("recovery failed"),
  );

  const first = render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  const options = shownToast();
  options.action.onClick();
  options.onDismiss();
  await vi.waitFor(() =>
    expect(mocks.requestCaptureRecovery).toHaveBeenCalledOnce(),
  );
  await act(async () => {});
  consoleError.mockRestore();
  first.unmount();
  mocks.toastWarning.mockClear();

  render(<SavedCaptureAudioPrompt sessionId="session-1" />);
  expect(mocks.toastWarning).toHaveBeenCalledOnce();
});
