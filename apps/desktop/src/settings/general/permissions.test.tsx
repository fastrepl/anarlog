import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Permission, PermissionStatus } from "@anlg/plugin-permissions";

const mocks = vi.hoisted(() => ({
  currentPlatform: "macos",
  permissions: new Map<
    Permission,
    {
      status: PermissionStatus;
      confirmedStatus: PermissionStatus;
      isPending: boolean;
      open: ReturnType<typeof vi.fn>;
      request: ReturnType<typeof vi.fn>;
      reset: ReturnType<typeof vi.fn>;
      error: string | null;
    }
  >(),
  usePermission: vi.fn(),
  guidance: null as { assisted: boolean; paneTitle: string | null } | null,
  closePermissionAssistant: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => mocks.currentPlatform,
}));

vi.mock("~/shared/hooks/usePermissions", () => ({
  usePermission: (permission: Permission) => {
    mocks.usePermission(permission);
    return mocks.permissions.get(permission);
  },
  usePermissionGuidance: () => mocks.guidance,
  closePermissionAssistant: mocks.closePermissionAssistant,
}));

import { Permissions } from "./permissions";

function permission(status: PermissionStatus) {
  return {
    status,
    confirmedStatus: status,
    isPending: false,
    open: vi.fn(),
    request: vi.fn(),
    reset: vi.fn(),
    error: null as string | null,
  };
}

function renderPermissions(accessibilityStatus: PermissionStatus) {
  const accessibility = permission(accessibilityStatus);

  mocks.permissions.set("microphone", permission("authorized"));
  mocks.permissions.set("systemAudio", permission("authorized"));
  mocks.permissions.set("accessibility", accessibility);
  mocks.permissions.set("calendar", permission("authorized"));

  render(<Permissions />);

  return accessibility;
}

describe("Permissions", () => {
  afterEach(() => {
    cleanup();
    mocks.currentPlatform = "macos";
    mocks.guidance = null;
    mocks.permissions.clear();
    mocks.usePermission.mockClear();
    mocks.closePermissionAssistant.mockClear();
  });

  it("explains what Accessibility enables and opens Settings when denied", () => {
    const accessibility = renderPermissions("denied");

    expect(
      screen.getByText(/Read meeting controls and visible chat/),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Open accessibility settings" }),
    );

    expect(accessibility.open).toHaveBeenCalledOnce();
    expect(accessibility.request).not.toHaveBeenCalled();
  });

  it("requests Accessibility before a permission decision exists", () => {
    const accessibility = renderPermissions("neverRequested");

    fireEvent.click(
      screen.getByRole("button", {
        name: "Request accessibility permission",
      }),
    );

    expect(accessibility.request).toHaveBeenCalledOnce();
    expect(accessibility.open).not.toHaveBeenCalled();
  });

  it("routes an assisted pane to the guided flow before any decision", () => {
    mocks.guidance = { assisted: true, paneTitle: "Accessibility" };
    const accessibility = renderPermissions("neverRequested");

    expect(
      screen.getByText(/guides you to add Anarlog to the Accessibility list/),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Open accessibility settings" }),
    );

    expect(accessibility.open).toHaveBeenCalledOnce();
    expect(accessibility.request).not.toHaveBeenCalled();
  });

  it("dismisses a lingering assistant when the section unmounts", () => {
    mocks.guidance = { assisted: true, paneTitle: "Accessibility" };
    renderPermissions("denied");

    expect(mocks.closePermissionAssistant).not.toHaveBeenCalled();

    cleanup();

    expect(mocks.closePermissionAssistant).toHaveBeenCalledOnce();
  });

  it("shows retryable audio capability checks outside macOS", () => {
    mocks.currentPlatform = "linux";
    const microphone = permission("denied");
    const systemAudio = permission("denied");
    microphone.error = "microphone device unavailable";
    systemAudio.error = "PipeWire source unavailable";
    mocks.permissions.set("microphone", microphone);
    mocks.permissions.set("systemAudio", systemAudio);

    render(<Permissions />);

    expect(screen.queryByText("Accessibility")).toBeNull();
    expect(screen.queryByText("Calendar")).toBeNull();
    expect(screen.getByText("microphone device unavailable")).toBeTruthy();
    expect(screen.getByText("PipeWire source unavailable")).toBeTruthy();
    expect(mocks.usePermission).not.toHaveBeenCalledWith("accessibility");
    expect(mocks.usePermission).not.toHaveBeenCalledWith("calendar");

    fireEvent.click(
      screen.getByRole("button", { name: "Try again: Microphone" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Try again: System audio" }),
    );

    expect(microphone.request).toHaveBeenCalledOnce();
    expect(systemAudio.request).toHaveBeenCalledOnce();
    expect(microphone.open).not.toHaveBeenCalled();
    expect(systemAudio.open).not.toHaveBeenCalled();
  });
});
