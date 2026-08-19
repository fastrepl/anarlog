import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  message: vi.fn(),
  error: vi.fn(),
  warning: vi.fn(),
  dismiss: vi.fn(),
}));

vi.mock("@anlg/ui/components/ui/toast", async (importOriginal) => {
  const actual =
    await importOriginal<typeof import("@anlg/ui/components/ui/toast")>();
  return {
    ...actual,
    sonnerToast: {
      message: mocks.message,
      error: mocks.error,
      warning: mocks.warning,
      dismiss: mocks.dismiss,
    },
  };
});

import { TOAST_DURATIONS } from "@anlg/ui/components/ui/toast";

import { SettingsAlertToast } from "./settings-alert";

describe("SettingsAlertToast", () => {
  beforeEach(() => {
    mocks.message.mockClear();
    mocks.error.mockClear();
    mocks.warning.mockClear();
    mocks.dismiss.mockClear();
  });

  afterEach(() => {
    cleanup();
  });

  it("shows persistent settings alerts through Sonner", () => {
    render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
        variant="error"
        lifecycle="persistent"
      />,
    );

    expect(mocks.error).toHaveBeenCalledWith("Provider not configured.", {
      id: "settings-alert",
      duration: TOAST_DURATIONS.error,
      dismissible: true,
      closeButton: true,
    });
  });

  it("dismisses its Sonner toast when the alert leaves the page", () => {
    const { unmount } = render(
      <SettingsAlertToast
        id="settings-alert"
        description="Provider not configured."
        lifecycle="condition-bound"
      />,
    );

    unmount();

    expect(mocks.dismiss).toHaveBeenCalledWith("settings-alert");
  });

  it("auto-dismisses error settings alerts while keeping actions available", () => {
    const onClick = vi.fn();

    render(
      <SettingsAlertToast
        id="keychain-alert"
        description="Repair Keychain access."
        variant="error"
        lifecycle="condition-bound"
        action={{ label: "Repair", onClick }}
      />,
    );

    expect(mocks.error).toHaveBeenCalledWith(
      "Repair Keychain access.",
      expect.objectContaining({
        id: "keychain-alert",
        duration: TOAST_DURATIONS.error,
        dismissible: false,
        closeButton: false,
        action: expect.objectContaining({ label: "Repair" }),
      }),
    );

    const options = mocks.error.mock.calls[0]?.[1] as {
      action: {
        onClick: (event: { preventDefault: () => void }) => void;
      };
    };
    const preventDefault = vi.fn();
    options.action.onClick({ preventDefault });

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(onClick).toHaveBeenCalledOnce();
    expect(mocks.dismiss).not.toHaveBeenCalled();
  });
});
