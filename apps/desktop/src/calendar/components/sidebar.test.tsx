import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PermissionStatus } from "@anlg/plugin-permissions";

type ContextMenuItem = {
  id?: string;
  text?: string;
  action?: () => void;
  separator?: true;
};

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
  removeDisconnectedCalendarConnection: vi.fn(),
  allowReconnectedCalendarConnections: vi.fn(),
  contextMenus: [] as ContextMenuItem[][],
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
  useNativeContextMenu: (items: ContextMenuItem[]) => {
    mocks.contextMenus.push(items);
    return vi.fn();
  },
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

vi.mock("~/services/calendar", () => ({
  removeDisconnectedCalendarConnection:
    mocks.removeDisconnectedCalendarConnection,
  allowReconnectedCalendarConnections:
    mocks.allowReconnectedCalendarConnections,
}));

vi.mock("./apple/calendar-selection", () => ({
  AppleCalendarSelection: () => null,
}));

import { CalendarSidebarContent } from "./sidebar";

function findContextMenuItem(id: string) {
  for (const items of mocks.contextMenus) {
    const match = items.find(
      (item) => !("separator" in item) && item.id === id,
    );
    if (match && !("separator" in match)) {
      return match;
    }
  }
  return undefined;
}

describe("CalendarSidebarContent", () => {
  afterEach(() => {
    cleanup();
    mocks.calendar.status = "denied";
    mocks.calendar.confirmedStatus = "denied";
    mocks.calendar.isPending = false;
    mocks.calendar.open.mockClear();
    mocks.calendar.request.mockClear();
    mocks.calendar.reset.mockClear();
    mocks.removeDisconnectedCalendarConnection.mockClear();
    mocks.allowReconnectedCalendarConnections.mockClear();
    mocks.contextMenus = [];
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

  it("offers reconnect and disconnect on the Apple Calendar row", () => {
    mocks.calendar.status = "authorized";
    mocks.calendar.confirmedStatus = "authorized";

    render(<CalendarSidebarContent />);

    expect(
      screen.getByRole("button", { name: "Open calendar account actions" }),
    ).toBeTruthy();

    const disconnect = findContextMenuItem("disconnect-apple-calendar");
    const reconnect = findContextMenuItem("reconnect-apple-calendar");
    expect(disconnect?.text).toBe("Disconnect");
    expect(reconnect?.text).toBe("Reconnect");

    disconnect?.action();

    expect(mocks.removeDisconnectedCalendarConnection).toHaveBeenCalledWith(
      "apple",
      "apple",
    );
    expect(mocks.calendar.reset).toHaveBeenCalledOnce();

    reconnect?.action();

    expect(mocks.allowReconnectedCalendarConnections).toHaveBeenCalledWith(
      "apple",
    );
    expect(mocks.calendar.request).toHaveBeenCalledOnce();
  });
});
