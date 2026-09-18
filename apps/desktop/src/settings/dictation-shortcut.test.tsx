import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  validate: vi.fn(),
  save: vi.fn(),
  cleanup: vi.fn(),
}));
vi.mock("@anlg/plugin-shortcut", () => ({
  commands: { validate: mocks.validate },
}));
vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "macos" }));
vi.mock("~/settings/queries", () => ({ setSettingValue: mocks.save }));
vi.mock("~/dictation/lifecycle", () => ({
  waitForDictationCleanup: mocks.cleanup,
}));
vi.mock("@lingui/react/macro", () => ({
  Trans: ({ children }: { children: React.ReactNode }) => children,
  useLingui: () => ({ t: (strings: TemplateStringsArray) => strings.join("") }),
}));

import { DictationShortcut } from "./dictation-shortcut";

import { useDictationStatus } from "~/dictation/state";

function setup() {
  return render(
    <QueryClientProvider
      client={
        new QueryClient({ defaultOptions: { mutations: { retry: false } } })
      }
    >
      <DictationShortcut shortcut="Control+Alt+Space" />
    </QueryClientProvider>,
  );
}

async function start() {
  const button = screen.getByRole("button", { name: "Dictation shortcut" });
  fireEvent.click(button);
  await screen.findByText("Press shortcut…");
  return button;
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.validate.mockResolvedValue({ status: "ok" });
  mocks.save.mockResolvedValue(undefined);
  mocks.cleanup.mockResolvedValue(undefined);
  useDictationStatus.setState({ capturingShortcut: false });
});
afterEach(cleanup);

describe("dictation shortcut recorder", () => {
  it("captures a combination and saves automatically", async () => {
    setup();
    const button = await start();
    expect(useDictationStatus.getState().capturingShortcut).toBe(true);
    fireEvent.keyDown(button, {
      key: "d",
      code: "KeyD",
      ctrlKey: true,
      altKey: true,
    });
    await waitFor(() =>
      expect(mocks.save).toHaveBeenCalledWith(
        "dictation_shortcut",
        "Control+Alt+KeyD",
      ),
    );
    await waitFor(() =>
      expect(useDictationStatus.getState().capturingShortcut).toBe(false),
    );
  });

  it.each(["Escape", "Tab", "blur", "unmount"])(
    "cancels on %s without saving",
    async (action) => {
      const view = setup();
      const button = await start();
      if (action === "unmount") view.unmount();
      else if (action === "blur") fireEvent.blur(button);
      else fireEvent.keyDown(button, { key: action, code: action });
      expect(useDictationStatus.getState().capturingShortcut).toBe(false);
      expect(mocks.save).not.toHaveBeenCalled();
    },
  );

  it("waits for global shortcut cleanup and ignores modifier-only keys", async () => {
    let finish!: () => void;
    mocks.cleanup.mockReturnValue(
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    );
    setup();
    const button = screen.getByRole("button", { name: "Dictation shortcut" });
    fireEvent.click(button);
    fireEvent.keyDown(button, { key: "d", code: "KeyD", ctrlKey: true });
    expect(mocks.validate).not.toHaveBeenCalled();
    finish();
    await screen.findByText("Press shortcut…");
    fireEvent.keyDown(button, {
      key: "Control",
      code: "ControlLeft",
      ctrlKey: true,
    });
    expect(mocks.validate).not.toHaveBeenCalled();
  });

  it("restores the global shortcut on validation failure", async () => {
    mocks.validate.mockResolvedValue({
      status: "error",
      error: "Choose a modifier.",
    });
    setup();
    fireEvent.keyDown(await start(), { key: "d", code: "KeyD" });
    expect((await screen.findByRole("alert")).textContent).toBe(
      "Choose a modifier.",
    );
    expect(useDictationStatus.getState().capturingShortcut).toBe(false);
    expect(mocks.save).not.toHaveBeenCalled();
  });

  it("stops offering cancellation once persistence starts", async () => {
    let complete!: () => void;
    mocks.save.mockReturnValue(
      new Promise<void>((resolve) => {
        complete = resolve;
      }),
    );
    setup();
    const button = await start();
    fireEvent.keyDown(button, { key: "d", code: "KeyD", ctrlKey: true });
    await waitFor(() =>
      expect((button as HTMLButtonElement).disabled).toBe(true),
    );
    expect(useDictationStatus.getState().capturingShortcut).toBe(false);
    fireEvent.keyDown(button, { key: "Escape" });
    fireEvent.blur(button);
    expect(button.getAttribute("aria-busy")).toBe("true");
    complete();
    await waitFor(() =>
      expect((button as HTMLButtonElement).disabled).toBe(false),
    );
    expect(mocks.save).toHaveBeenCalledTimes(1);
  });

  it("does not save a validation result after capture is cancelled", async () => {
    let finish!: (result: { status: string }) => void;
    mocks.validate.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    setup();
    const button = await start();
    fireEvent.keyDown(button, { key: "d", code: "KeyD", ctrlKey: true });
    await waitFor(() => expect(mocks.validate).toHaveBeenCalled());
    fireEvent.keyDown(button, { key: "Escape" });
    finish({ status: "ok" });
    await waitFor(() =>
      expect(useDictationStatus.getState().capturingShortcut).toBe(false),
    );
    expect(mocks.save).not.toHaveBeenCalled();
  });
});
