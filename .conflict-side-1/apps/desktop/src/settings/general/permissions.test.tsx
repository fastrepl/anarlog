import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { Permission, PermissionStatus } from "@hypr/plugin-permissions";

const mocks = vi.hoisted(() => ({
  permissions: new Map<
    Permission,
    {
      status: PermissionStatus;
      isPending: boolean;
      open: ReturnType<typeof vi.fn>;
      request: ReturnType<typeof vi.fn>;
      reset: ReturnType<typeof vi.fn>;
    }
  >(),
}));

vi.mock("~/shared/hooks/usePermissions", () => ({
  usePermission: (permission: Permission) => mocks.permissions.get(permission),
}));

import { Permissions } from "./permissions";

function permission(status: PermissionStatus) {
  return {
    status,
    isPending: false,
    open: vi.fn(),
    request: vi.fn(),
    reset: vi.fn(),
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
    mocks.permissions.clear();
  });

  it("explains what Accessibility enables and opens Settings when denied", () => {
    const accessibility = renderPermissions("denied");

    expect(
      screen.getByText(
        /Required to read meeting controls, visible chat, and participant status/,
      ),
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
});
