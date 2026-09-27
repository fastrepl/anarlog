import { Channel } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

import type { QueryEvent } from "@anlg/plugin-db";

const { subscribe } =
  await vi.importActual<typeof import("@anlg/plugin-db")>("@anlg/plugin-db");

type ChannelMessage = { index: number } & (
  | { message: QueryEvent<{ id: string }> }
  | { end: true }
);

describe("Tauri live-query channel", () => {
  let deliver: (message: ChannelMessage) => void;
  let channel: Channel<QueryEvent<{ id: string }>>;
  let unregister: ReturnType<typeof vi.fn>;
  let invoke: ReturnType<
    typeof vi.fn<
      (command: string, payload?: Record<string, unknown>) => Promise<unknown>
    >
  >;

  beforeEach(() => {
    unregister = vi.fn();
    invoke = vi.fn();
    vi.stubGlobal("__TAURI_INTERNALS__", {
      transformCallback: (callback: typeof deliver) => {
        deliver = callback;
        return 123;
      },
      unregisterCallback: unregister,
      invoke,
    });
    invoke.mockImplementation(async (command, payload) => {
      if (command === "plugin:db|subscribe") {
        const onEvent = payload?.onEvent;
        if (!(onEvent instanceof Channel)) {
          throw new Error("Expected a real Tauri channel");
        }
        channel = onEvent;
        return {
          id: "query-1",
          analysis: { kind: "reactive", data: {} },
        };
      }
    });
    vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  test.each(["result", "error"] as const)(
    "a throwing %s callback cannot stall ordered delivery or channel cleanup",
    async (event) => {
      const failure = new Error("consumer failed");
      const onData = vi.fn();
      const onError = vi.fn();
      (event === "result" ? onData : onError).mockImplementationOnce(() => {
        throw failure;
      });
      const stop = await subscribe("SELECT id FROM templates", [], {
        onData,
        onError,
      });

      deliver({
        index: 1,
        message: { event: "result", data: [{ id: "newer" }] },
      });
      expect(() =>
        deliver({
          index: 0,
          message:
            event === "result"
              ? { event: "result", data: [{ id: "older" }] }
              : { event: "error", data: "query failed" },
        }),
      ).not.toThrow();
      expect(onData).toHaveBeenLastCalledWith([{ id: "newer" }]);
      expect(console.error).toHaveBeenCalledWith(
        "[plugin-db] live query callback failed",
        failure,
      );

      await stop();
      deliver({ index: 2, end: true });
      expect(unregister).toHaveBeenCalledWith(channel.id);
    },
  );

  test("unsubscribe detaches consumers before native cleanup starts", async () => {
    const onData = vi.fn();
    const stop = await subscribe("SELECT id FROM templates", [], { onData });
    invoke.mockImplementationOnce(async (command) => {
      expect(command).toBe("plugin:db|unsubscribe");
      deliver({
        index: 0,
        message: { event: "result", data: [{ id: "late" }] },
      });
      deliver({ index: 1, end: true });
    });

    await stop();
    expect(onData).not.toHaveBeenCalled();
    expect(unregister).toHaveBeenCalledWith(channel.id);
  });

  test("failed setup detaches consumers while the native channel closes", async () => {
    const onData = vi.fn();
    invoke.mockRejectedValueOnce(new Error("setup failed"));
    await expect(
      subscribe("SELECT id FROM templates", [], { onData }),
    ).rejects.toThrow("setup failed");

    deliver({
      index: 0,
      message: { event: "result", data: [{ id: "late" }] },
    });
    deliver({ index: 1, end: true });
    expect(onData).not.toHaveBeenCalled();
    expect(unregister).toHaveBeenCalledWith(123);
  });
});
