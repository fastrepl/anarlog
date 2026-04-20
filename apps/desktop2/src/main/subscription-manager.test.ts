import { EventEmitter } from "node:events";
import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the NAPI SDK so we can drive delta callbacks by hand and count
// subscribe/unsubscribe invocations. The subscription manager must treat
// `@hypr/napi-sdk` as a thin primitive — one handle per `(sql, params)`.
type Listener = (delta: {
  event: string;
  rows?: unknown[];
  error?: string;
  reactive: boolean;
}) => void;

type FakeHandle = {
  reactive: boolean;
  unsubscribe: ReturnType<typeof vi.fn>;
  _emit: Listener;
};

const handles: FakeHandle[] = [];
const subscribeMock = vi.fn((sql: string, params: unknown[], cb: Listener) => {
  const handle: FakeHandle = {
    reactive: !sql.includes("-- non-reactive"),
    unsubscribe: vi.fn(),
    _emit: cb,
  };
  handles.push(handle);
  return handle;
});

vi.mock("@hypr/napi-sdk", () => ({
  subscribe: (sql: string, params: unknown[], cb: Listener) =>
    subscribeMock(sql, params, cb),
}));

// Import AFTER the mock so the manager picks up the fake.
const { LiveQuerySubscriptionManager } =
  await import("./subscription-manager.js");

// Minimal `WebContents`-like fake. The manager only reads `.id`, calls
// `.send`, checks `.isDestroyed()`, and attaches `.once("destroyed", cb)` —
// an EventEmitter covers all of that.
class FakeWebContents extends EventEmitter {
  public readonly id: number;
  public readonly send = vi.fn();
  public isDestroyed = vi.fn(() => this._destroyed);
  private _destroyed = false;

  constructor(id: number) {
    super();
    this.id = id;
  }

  destroy(): void {
    this._destroyed = true;
    this.emit("destroyed");
  }
}

function emitSnapshot(handle: FakeHandle, rows: unknown[]) {
  handle._emit({ event: "snapshot", rows, reactive: handle.reactive });
}

function emitError(handle: FakeHandle, error: string) {
  handle._emit({ event: "error", error, reactive: handle.reactive });
}

beforeEach(() => {
  handles.length = 0;
  subscribeMock.mockClear();
});

describe("LiveQuerySubscriptionManager", () => {
  it("dedupes NAPI handles per (sql, params) across listeners", () => {
    const manager = new LiveQuerySubscriptionManager();
    const a = new FakeWebContents(1) as unknown as Electron.WebContents;
    const b = new FakeWebContents(2) as unknown as Electron.WebContents;

    manager.start("SELECT 1", [], a);
    manager.start("SELECT 1", [], b);

    expect(subscribeMock).toHaveBeenCalledTimes(1);
  });

  it("mints distinct private channels per listener and fans out per-subscriber", () => {
    const manager = new LiveQuerySubscriptionManager();
    const a = new FakeWebContents(1);
    const b = new FakeWebContents(2);

    const first = manager.start(
      "SELECT 1",
      [],
      a as unknown as Electron.WebContents,
    );
    const second = manager.start(
      "SELECT 1",
      [],
      b as unknown as Electron.WebContents,
    );

    expect(first.channel).not.toEqual(second.channel);

    emitSnapshot(handles[0]!, [{ n: 1 }]);

    expect(a.send).toHaveBeenCalledWith(first.channel, {
      event: "result",
      data: [{ n: 1 }],
    });
    expect(b.send).toHaveBeenCalledWith(second.channel, {
      event: "result",
      data: [{ n: 1 }],
    });
    // Neither sender receives the other's channel: symmetric check.
    const aChannels = a.send.mock.calls.map(([channel]) => channel);
    const bChannels = b.send.mock.calls.map(([channel]) => channel);
    expect(aChannels).not.toContain(second.channel);
    expect(bChannels).not.toContain(first.channel);
  });

  it("reports non-reactive analysis from the underlying handle", () => {
    const manager = new LiveQuerySubscriptionManager();
    const sender = new FakeWebContents(1) as unknown as Electron.WebContents;

    const { reactive } = manager.start("SELECT 1 -- non-reactive", [], sender);

    expect(reactive).toBe(false);
  });

  it("releases the NAPI handle only when the last listener stops", () => {
    const manager = new LiveQuerySubscriptionManager();
    const a = new FakeWebContents(1);
    const b = new FakeWebContents(2);

    const first = manager.start(
      "SELECT 1",
      [],
      a as unknown as Electron.WebContents,
    );
    const second = manager.start(
      "SELECT 1",
      [],
      b as unknown as Electron.WebContents,
    );

    manager.stopByChannel(first.channel);
    expect(handles[0]!.unsubscribe).not.toHaveBeenCalled();

    manager.stopByChannel(second.channel);
    expect(handles[0]!.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("stops all of a window's subscriptions when its webContents is destroyed", () => {
    const manager = new LiveQuerySubscriptionManager();
    const sender = new FakeWebContents(1);

    manager.start("SELECT 1", [], sender as unknown as Electron.WebContents);
    manager.start("SELECT 2", [], sender as unknown as Electron.WebContents);

    expect(handles).toHaveLength(2);

    sender.destroy();

    expect(handles[0]!.unsubscribe).toHaveBeenCalledTimes(1);
    expect(handles[1]!.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("normalizes SDK deltas into QueryEvent shape (result / error)", () => {
    const manager = new LiveQuerySubscriptionManager();
    const sender = new FakeWebContents(1);

    const { channel } = manager.start(
      "SELECT 1",
      [],
      sender as unknown as Electron.WebContents,
    );

    emitSnapshot(handles[0]!, [{ n: 1 }]);
    emitError(handles[0]!, "boom");

    expect(sender.send).toHaveBeenNthCalledWith(1, channel, {
      event: "result",
      data: [{ n: 1 }],
    });
    expect(sender.send).toHaveBeenNthCalledWith(2, channel, {
      event: "error",
      data: "boom",
    });
  });

  it("reaps dead subscribers when send fails", () => {
    const manager = new LiveQuerySubscriptionManager();
    const sender = new FakeWebContents(1);
    sender.send.mockImplementation(() => {
      throw new Error("disconnected");
    });

    manager.start("SELECT 1", [], sender as unknown as Electron.WebContents);

    emitSnapshot(handles[0]!, [{ n: 1 }]);

    expect(handles[0]!.unsubscribe).toHaveBeenCalledTimes(1);
  });

  it("destroy() tears down every live NAPI handle", () => {
    const manager = new LiveQuerySubscriptionManager();
    const a = new FakeWebContents(1);
    const b = new FakeWebContents(2);

    manager.start("SELECT 1", [], a as unknown as Electron.WebContents);
    manager.start("SELECT 2", [], b as unknown as Electron.WebContents);

    manager.destroy();

    expect(handles[0]!.unsubscribe).toHaveBeenCalledTimes(1);
    expect(handles[1]!.unsubscribe).toHaveBeenCalledTimes(1);
  });
});
