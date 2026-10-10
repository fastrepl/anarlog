import { afterEach, expect, test, vi } from "vitest";

const clients = ["work", "home"].map((fingerprint, index) => ({
  fingerprint,
  preferred: index === 0 ? "home" : "work",
  live: { sessionId: `conflict-${fingerprint}`, status: "active" },
  listeners: new Set<() => void>(),
  toast: Object.assign(vi.fn(), { info: vi.fn(), dismiss: vi.fn() }),
}));

afterEach(() => {
  for (const client of clients) {
    client.live.status = "inactive";
    for (const listener of [...client.listeners]) listener();
  }
  vi.useRealTimers();
});

test("competing coordinators keep capture until a shared claim selects one winner", async () => {
  vi.useFakeTimers();
  let winner = "";
  const present = new Set<string>();
  const coordinators = [];
  for (const client of clients) {
    vi.resetModules();
    vi.doMock("@anlg/ui/components/ui/toast", () => ({ toast: client.toast }));
    vi.doMock("~/auth/client", () => ({
      supabase: {
        auth: {
          getSession: async () => ({
            data: {
              session: {
                access_token: "fixture",
                user: { is_anonymous: false },
              },
            },
          }),
        },
      },
    }));
    vi.doMock("~/auth/cloudsync-credentials", () => ({
      getDeviceIdentity: async () => ({ fingerprint: client.fingerprint }),
    }));
    vi.doMock("~/settings/queries", () => ({
      getStoredSettingValues: async () => ({
        values: { primary_recording_device: client.preferred },
        hasValues: new Set(["primary_recording_device"]),
      }),
    }));
    vi.doMock("~/auth/sync-devices", () => ({
      requestMeetingDevices: async ({
        fingerprint,
        intent,
      }: {
        fingerprint: string;
        intent: string;
      }) => {
        if (intent === "release") present.delete(fingerprint);
        else present.add(fingerprint);
        if (intent === "claim") winner = fingerprint;
        return clients
          .filter((device) => present.has(device.fingerprint))
          .map((device) => ({
            deviceFingerprint: device.fingerprint,
            deviceName: device.fingerprint,
            primary: device.fingerprint === winner,
          }));
      },
    }));
    vi.doMock("~/store/zustand/listener/instance", () => ({
      listenerStore: {
        getState: () => ({
          live: client.live,
          stop: () => {
            client.live.status = "inactive";
            for (const listener of [...client.listeners]) listener();
          },
        }),
        subscribe: (listener: () => void) => {
          client.listeners.add(listener);
          return () => client.listeners.delete(listener);
        },
      },
    }));
    const coordinator = await import("./primary-device");
    coordinators.push(coordinator);
    coordinator.startPrimaryDeviceCoordination({
      sessionId: client.live.sessionId,
      event: {
        tracking_id: "shared-meeting",
        started_at: "2026-10-10T10:00:00Z",
      },
      automatic: true,
    });
    await vi.waitFor(() => expect(present.has(client.fingerprint)).toBe(true));
  }
  await vi.advanceTimersByTimeAsync(
    coordinators[0].PRIMARY_DEVICE_HEARTBEAT_MS,
  );
  expect(clients.map((client) => client.live.status)).toEqual([
    "active",
    "active",
  ]);
  const prompts = clients[0].toast.mock.calls;
  prompts[prompts.length - 1][1].action.onClick();
  await vi.advanceTimersByTimeAsync(0);
  expect(winner).toBe("work");
  await vi.advanceTimersByTimeAsync(
    coordinators[1].PRIMARY_DEVICE_HEARTBEAT_MS,
  );
  expect(clients.map((client) => client.live.status)).toEqual([
    "active",
    "inactive",
  ]);
  expect(coordinators[0].consumePrimaryDeviceYield("conflict-work")).toBe(
    false,
  );
  expect(coordinators[1].consumePrimaryDeviceYield("conflict-home")).toBe(true);
});
