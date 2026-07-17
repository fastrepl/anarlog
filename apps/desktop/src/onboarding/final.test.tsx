import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  analyticsEvent: vi.fn(),
  createSession: vi.fn(),
  flushAutomaticRelaunch: vi.fn(),
  getOrCreateWelcomeSession: vi.fn(),
  setOnboardingNeeded: vi.fn(),
  setPendingWelcomeSession: vi.fn(),
  stopSfx: vi.fn(),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: { event: mocks.analyticsEvent },
}));

vi.mock("@hypr/plugin-opener2", () => ({
  commands: { openUrl: vi.fn() },
}));

vi.mock("@hypr/plugin-sfx", () => ({
  commands: { stop: mocks.stopSfx },
}));

vi.mock("./welcome-note", () => ({
  getOrCreateWelcomeSession: mocks.getOrCreateWelcomeSession,
  setPendingWelcomeSession: mocks.setPendingWelcomeSession,
}));

vi.mock("~/session/queries", () => ({
  createSession: mocks.createSession,
}));

vi.mock("~/shared/relaunch", () => ({
  flushAutomaticRelaunch: mocks.flushAutomaticRelaunch,
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { setOnboardingNeeded: mocks.setOnboardingNeeded },
}));

import { FinalSection, finishOnboarding } from "./final";

beforeEach(() => {
  vi.clearAllMocks();
  mocks.analyticsEvent.mockResolvedValue(null);
  mocks.flushAutomaticRelaunch.mockResolvedValue(false);
  mocks.getOrCreateWelcomeSession.mockResolvedValue("welcome-session");
  mocks.setOnboardingNeeded.mockResolvedValue({ status: "ok", data: null });
  mocks.stopSfx.mockResolvedValue(null);
});

afterEach(cleanup);

it("opens a blank note when welcome-note creation fails", async () => {
  const onContinue = vi.fn();
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.getOrCreateWelcomeSession.mockRejectedValueOnce(
    new Error("malformed JSON"),
  );
  mocks.createSession.mockResolvedValueOnce("blank-session");

  await finishOnboarding(onContinue);

  expect(mocks.createSession).toHaveBeenCalledTimes(1);
  expect(onContinue).toHaveBeenCalledWith("blank-session");
  consoleError.mockRestore();
});

it("shows a retryable error when onboarding cannot be persisted", async () => {
  const onContinue = vi.fn();
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.setOnboardingNeeded.mockResolvedValueOnce({
    status: "error",
    error: "settings unavailable",
  });

  render(<FinalSection onContinue={onContinue} />);
  fireEvent.click(screen.getByRole("button", { name: "Open Anarlog" }));

  expect(
    (
      screen.getByRole("button", {
        name: "Open Anarlog",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await waitFor(() => {
    expect(screen.getByRole("alert").textContent).toBe(
      "Couldn't open Anarlog. Please try again.",
    );
  });
  expect(
    (
      screen.getByRole("button", {
        name: "Open Anarlog",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(false);
  expect(onContinue).not.toHaveBeenCalled();
  consoleError.mockRestore();
});

it("reuses the blank fallback session when persistence is retried", async () => {
  const onContinue = vi.fn();
  const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
  mocks.getOrCreateWelcomeSession.mockRejectedValue(
    new Error("malformed JSON"),
  );
  mocks.createSession.mockResolvedValue("blank-session");
  mocks.setOnboardingNeeded
    .mockResolvedValueOnce({ status: "error", error: "settings unavailable" })
    .mockResolvedValueOnce({ status: "ok", data: null });

  render(<FinalSection onContinue={onContinue} />);
  fireEvent.click(screen.getByRole("button", { name: "Open Anarlog" }));
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "Open Anarlog" }));

  await waitFor(() => {
    expect(onContinue).toHaveBeenCalledWith("blank-session");
  });
  expect(mocks.createSession).toHaveBeenCalledTimes(1);
  consoleError.mockRestore();
});

it("ignores concurrent finish attempts", async () => {
  const onContinue = vi.fn();
  let resolveWelcomeSession: (sessionId: string) => void = () => {};
  mocks.getOrCreateWelcomeSession.mockReturnValue(
    new Promise((resolve) => {
      resolveWelcomeSession = resolve;
    }),
  );

  render(<FinalSection onContinue={onContinue} />);
  const button = screen.getByRole("button", { name: "Open Anarlog" });
  fireEvent.click(button);
  fireEvent.click(button);
  resolveWelcomeSession("welcome-session");

  await waitFor(() => {
    expect(onContinue).toHaveBeenCalledWith("welcome-session");
  });
  expect(mocks.getOrCreateWelcomeSession).toHaveBeenCalledTimes(1);
  expect(onContinue).toHaveBeenCalledTimes(1);
});
