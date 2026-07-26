import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => {
  const createPermission = () => ({
    status: "denied" as "authorized" | "denied" | "neverRequested",
    isPending: false,
    open: vi.fn(),
    request: vi.fn(),
    reset: vi.fn(),
  });
  const permissions = {
    microphone: createPermission(),
    systemAudio: createPermission(),
    accessibility: createPermission(),
  };

  return {
    currentPlatform: "macos",
    permissions,
    usePermission: vi.fn((type: keyof typeof permissions) => permissions[type]),
  };
});

const lingui = vi.hoisted(() => ({
  t: (input: TemplateStringsArray, ...values: unknown[]) =>
    input.reduce(
      (message, part, index) =>
        `${message}${part}${index < values.length ? String(values[index]) : ""}`,
      "",
    ),
}));

vi.mock("@lingui/react/macro", () => ({
  useLingui: () => ({ t: lingui.t }),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.currentPlatform,
}));

vi.mock("~/shared/hooks/usePermissions", () => ({
  usePermission: mocks.usePermission,
}));

import { PermissionsSection } from "./permissions";

afterEach(cleanup);

describe("PermissionsSection", () => {
  beforeEach(() => {
    mocks.currentPlatform = "macos";
    vi.clearAllMocks();

    Object.values(mocks.permissions).forEach((permission) => {
      permission.status = "denied";
      permission.isPending = false;
    });
  });

  it("collects Accessibility permission on macOS", () => {
    const { container } = render(<PermissionsSection />);

    expect(screen.getByText("Help Anarlog listen to you")).toBeTruthy();
    expect(screen.getByText("Help Anarlog listen to others")).toBeTruthy();
    expect(screen.getByText("Help Anarlog read meeting activity")).toBeTruthy();
    expect(
      screen
        .getByRole("button", { name: "Enable accessibility" })
        .getAttribute("title"),
    ).toBe("Read meeting controls, visible chat, and participant status");
    expect(container.querySelectorAll(".lucide-arrow-right")).toHaveLength(3);
  });

  it("waits for all three macOS permissions before continuing", () => {
    const onContinue = vi.fn();
    mocks.permissions.microphone.status = "authorized";
    mocks.permissions.systemAudio.status = "authorized";

    const view = render(<PermissionsSection onContinue={onContinue} />);

    expect(onContinue).not.toHaveBeenCalled();

    mocks.permissions.accessibility.status = "authorized";
    view.rerender(<PermissionsSection onContinue={onContinue} />);

    expect(onContinue).toHaveBeenCalledTimes(1);

    view.rerender(<PermissionsSection onContinue={onContinue} />);

    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it("preserves the audio-only flow outside macOS", () => {
    const onContinue = vi.fn();
    mocks.currentPlatform = "windows";
    mocks.permissions.microphone.status = "authorized";
    mocks.permissions.systemAudio.status = "authorized";

    render(<PermissionsSection onContinue={onContinue} />);

    expect(screen.queryByText("Help Anarlog read meeting activity")).toBeNull();
    expect(mocks.usePermission).not.toHaveBeenCalledWith("accessibility");
    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it("requests denied Accessibility permission instead of opening Settings", () => {
    render(<PermissionsSection />);

    fireEvent.click(
      screen.getByRole("button", { name: "Enable accessibility" }),
    );

    expect(mocks.permissions.accessibility.request).toHaveBeenCalledTimes(1);
    expect(mocks.permissions.accessibility.open).not.toHaveBeenCalled();
  });
});
