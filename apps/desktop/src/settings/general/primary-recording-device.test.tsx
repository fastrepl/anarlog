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
  getDeviceIdentity: vi.fn(
    async (): Promise<{ fingerprint: string | null; name: string }> => ({
      fingerprint: "work-device",
      name: "Work Mac",
    }),
  ),
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
vi.mock("~/auth/cloudsync-credentials", () => ({
  getDeviceIdentity: mocks.getDeviceIdentity,
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

test("offers only the current desktop and can clear its preference", async () => {
  mocks.requestSyncDevices.mockResolvedValue({
    devices: [
      { deviceFingerprint: "work-device", deviceName: "Work Mac" },
      { deviceFingerprint: "phone-device", deviceName: "My phone" },
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
  expect(screen.queryByRole("option", { name: "My phone" })).toBeNull();
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

test("identity lookup failure can clear a preference and retry without remounting", async () => {
  mocks.value = "work-device";
  mocks.getDeviceIdentity.mockResolvedValueOnce({
    fingerprint: null,
    name: "Work Mac",
  });
  mocks.requestSyncDevices.mockResolvedValue({
    devices: [{ deviceFingerprint: "work-device", deviceName: "Work Mac" }],
    pendingDevices: [],
    maxDevices: 3,
  });
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  Object.defineProperty(Element.prototype, "scrollIntoView", {
    configurable: true,
    value: vi.fn(),
  });
  render(
    <QueryClientProvider client={queryClient}>
      <PrimaryRecordingDeviceSelector />
    </QueryClientProvider>,
  );
  await screen.findByRole("alert");
  const trigger = screen.getByRole("combobox", {
    name: "Primary recording device",
  });
  expect(trigger).toHaveProperty("disabled", false);
  fireEvent.keyDown(trigger, { key: "Enter" });
  fireEvent.click(screen.getByRole("option", { name: "Ask each meeting" }));
  await waitFor(() =>
    expect(trigger.textContent).toContain("Ask each meeting"),
  );
  fireEvent.click(screen.getByRole("button", { name: "Try again" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  fireEvent.keyDown(trigger, { key: "Enter" });
  fireEvent.click(screen.getByRole("option", { name: "Work Mac" }));
  await waitFor(() => expect(trigger.textContent).toContain("Work Mac"));
  queryClient.clear();
});
