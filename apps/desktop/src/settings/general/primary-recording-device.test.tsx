import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  value: "",
  requestSyncDevices: vi.fn(),
  setSettingValue: vi.fn(async (_key: string, value: string) => {
    mocks.value = value;
  }),
}));

vi.mock("~/auth", () => ({
  useAuth: () => ({
    session: { access_token: "token", user: { id: "user-1" } },
  }),
}));
vi.mock("~/auth/billing-context", () => ({
  useBillingAccess: () => ({ isPro: true }),
}));
vi.mock("~/auth/sync-devices", () => ({
  requestSyncDevices: mocks.requestSyncDevices,
}));
vi.mock("~/shared/config", () => ({ useConfigValue: () => mocks.value }));
vi.mock("~/settings/queries", () => ({
  setSettingValue: mocks.setSettingValue,
}));

import { PrimaryRecordingDeviceSelector } from "./primary-recording-device";

const originalScrollIntoView = Object.getOwnPropertyDescriptor(
  Element.prototype,
  "scrollIntoView",
);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  mocks.value = "";
  if (originalScrollIntoView) {
    Object.defineProperty(
      Element.prototype,
      "scrollIntoView",
      originalScrollIntoView,
    );
  } else {
    Reflect.deleteProperty(Element.prototype, "scrollIntoView");
  }
});

test("chooses a device and can return to asking each meeting", async () => {
  mocks.requestSyncDevices.mockResolvedValue({
    devices: [
      { deviceFingerprint: "work-device", deviceName: "Work Mac" },
      { deviceFingerprint: "home-device", deviceName: "Home Mac" },
    ],
    pendingDevices: [],
    maxDevices: 3,
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  const scrollIntoView = vi.fn();
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView,
  });
  render(
    <QueryClientProvider client={queryClient}>
      <PrimaryRecordingDeviceSelector />
    </QueryClientProvider>,
  );

  const trigger = screen.getByRole("combobox", {
    name: "Primary recording device",
  });
  await waitFor(() => expect(trigger).toHaveProperty("disabled", false));
  fireEvent.keyDown(trigger, { key: "Enter" });
  fireEvent.click(screen.getByRole("option", { name: "Work Mac" }));
  await waitFor(() => expect(trigger.textContent).toContain("Work Mac"));
  expect(mocks.setSettingValue).toHaveBeenLastCalledWith(
    "primary_recording_device",
    "work-device",
  );

  fireEvent.keyDown(trigger, { key: "Enter" });
  fireEvent.click(screen.getByRole("option", { name: "Ask each meeting" }));
  await waitFor(() =>
    expect(trigger.textContent).toContain("Ask each meeting"),
  );
  expect(mocks.setSettingValue).toHaveBeenLastCalledWith(
    "primary_recording_device",
    "",
  );
  queryClient.clear();
});
