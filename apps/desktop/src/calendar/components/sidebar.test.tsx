import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { createElement } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { PermissionStatus } from "@anlg/plugin-permissions";

// The real iconify-icon web component renders asynchronously via timers that
// can fire after the test environment is torn down ("document is not
// defined" unhandled errors), so render an inert element instead.
vi.mock("@iconify-icon/react", () => ({
  Icon: (props: Record<string, unknown>) =>
    createElement("iconify-icon", props),
}));

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
  syncCalendarEvents: vi.fn(),
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
  syncCalendarEvents: mocks.syncCalendarEvents,
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
    mocks.removeDisconnectedCalendarConnection.mockReset();
    mocks.removeDisconnectedCalendarConnection.mockResolvedValue(undefined);
    mocks.allowReconnectedCalendarConnections.mockClear();
    mocks.syncCalendarEvents.mockReset();
    mocks.syncCalendarEvents.mockResolvedValue(undefined);
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

  it("offers reconnect and disconnect on the Apple Calendar row", async () => {
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

    disconnect?.action?.();

    expect(mocks.removeDisconnectedCalendarConnection).toHaveBeenCalledWith(
      "apple",
      "apple",
    );
    await waitFor(() => {
      expect(mocks.calendar.reset).toHaveBeenCalledOnce();
    });
    expect(mocks.syncCalendarEvents).toHaveBeenCalledOnce();

    reconnect?.action?.();

    expect(mocks.allowReconnectedCalendarConnections).toHaveBeenCalledWith(
      "apple",
    );
    expect(mocks.calendar.request).toHaveBeenCalledOnce();
  });

  it("keeps Apple Calendar connected when disconnect persistence fails", async () => {
    mocks.calendar.status = "authorized";
    mocks.calendar.confirmedStatus = "authorized";
    mocks.removeDisconnectedCalendarConnection.mockRejectedValueOnce(
      new Error("write failed"),
    );

    render(<CalendarSidebarContent />);

    const disconnect = findContextMenuItem("disconnect-apple-calendar");
    disconnect?.action?.();

    await waitFor(() => {
      expect(mocks.syncCalendarEvents).toHaveBeenCalledOnce();
    });
    expect(mocks.calendar.reset).not.toHaveBeenCalled();
  });
});
