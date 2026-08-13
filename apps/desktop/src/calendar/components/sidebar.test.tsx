import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PermissionStatus } from "@anlg/plugin-permissions";

const mocks = vi.hoisted(() => ({
  calendar: {
    status: "denied" as PermissionStatus,
    confirmedStatus: "denied" as PermissionStatus,
    isPending: false,
    open: vi.fn(),
    request: vi.fn(),
    reset: vi.fn(),
    error: null as string | null,
  },
  openIntegration: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-os", () => ({
  platform: () => "macos",
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({ session: {} }),
}));

vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({
    isPaid: true,
    isPro: true,
    upgradeToPro: vi.fn(),
    isUpgradingToPro: false,
  }),
}));

vi.mock("~/auth/useConnections", () => ({
  useConnections: () => ({
    data: [],
    isPending: false,
    isError: false,
  }),
}));

vi.mock("~/shared/hooks/useNativeContextMenu", () => ({
  useNativeContextMenu: () => vi.fn(),
}));

vi.mock("~/shared/hooks/usePermissions", () => ({
  usePermission: () => mocks.calendar,
}));

vi.mock("~/shared/integration", () => ({
  openIntegrationUrl: vi.fn(),
  useOpenIntegrationUrl: () => ({
    openIntegration: mocks.openIntegration,
    openingAction: null,
  }),
}));

import { CalendarSidebarContent } from "./sidebar";

describe("CalendarSidebarContent", () => {
  afterEach(() => {
    cleanup();
    mocks.calendar.status = "denied";
    mocks.calendar.confirmedStatus = "denied";
    mocks.calendar.open.mockClear();
    mocks.calendar.request.mockClear();
  });

  it("explains how to recover after Apple Calendar access is denied", () => {
    render(<CalendarSidebarContent />);

    fireEvent.click(
      screen.getByRole("button", { name: "Connect Apple Calendar" }),
    );

    expect(screen.getByText("Apple Calendar access is off")).toBeTruthy();
    expect(mocks.calendar.open).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Open Settings" }));

    expect(mocks.calendar.open).toHaveBeenCalledOnce();
  });

  it("uses the native prompt before Apple Calendar access is decided", () => {
    mocks.calendar.status = "neverRequested";
    mocks.calendar.confirmedStatus = "neverRequested";

    render(<CalendarSidebarContent />);

    fireEvent.click(
      screen.getByRole("button", { name: "Connect Apple Calendar" }),
    );

    expect(mocks.calendar.request).toHaveBeenCalledOnce();
    expect(screen.queryByText("Apple Calendar access is off")).toBeNull();
  });
});
