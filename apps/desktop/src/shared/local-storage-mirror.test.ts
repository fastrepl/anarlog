import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, () => void>(),
  mirrorLocalStorage:
    vi.fn<(entries: Record<string, string>) => Promise<unknown>>(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: () => void) => {
    mocks.listeners.set(event, handler);
    return vi.fn();
  }),
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { mirrorLocalStorage: mocks.mirrorLocalStorage },
}));

const PENDING_KEY = "anarlog.cloud-api-pending.v1.user-1";

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  localStorage.clear();
  mocks.listeners.clear();
  mocks.mirrorLocalStorage.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

it("never lands an older pending-delete snapshot after a newer one", async () => {
  const { flushLocalStorageMirror, mirrorLocalStorage } =
    await import("./local-storage-mirror");
  let finishFirst = () => {};
  mocks.mirrorLocalStorage
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishFirst = () => resolve({ status: "ok", data: null });
        }),
    )
    .mockResolvedValue({ status: "ok", data: null });

  localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a"]}');
  mirrorLocalStorage();
  await vi.advanceTimersByTimeAsync(1_000);
  localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a","b"]}');
  const flushed = flushLocalStorageMirror();
  await vi.advanceTimersByTimeAsync(0);
  expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(1);

  finishFirst();
  await flushed;

  expect(
    mocks.mirrorLocalStorage.mock.calls.map(([entries]) => entries),
  ).toEqual([
    { [PENDING_KEY]: '{"upserts":[],"deletes":["a"]}' },
    { [PENDING_KEY]: '{"upserts":[],"deletes":["a","b"]}' },
  ]);
});

it("mirrors immediately at startup, flushes on Rust event, and debounces later changes", async () => {
  const { mirrorLocalStorage, startLocalStorageMirror } =
    await import("./local-storage-mirror");
  mocks.mirrorLocalStorage.mockResolvedValue({ status: "ok", data: null });

  try {
    localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a"]}');
    startLocalStorageMirror();
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(1);

    localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a","b"]}');
    mirrorLocalStorage();
    localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a","b","c"]}');
    mirrorLocalStorage();
    await vi.advanceTimersByTimeAsync(999);
    expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1);
    expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(2);

    localStorage.setItem(
      PENDING_KEY,
      '{"upserts":[],"deletes":["a","b","c","d"]}',
    );
    mirrorLocalStorage();
    const flushListener = mocks.listeners.get("updater2:flush-webview-storage");
    expect(flushListener).toBeTypeOf("function");
    flushListener?.();
    await vi.advanceTimersByTimeAsync(0);
    expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(3);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(mocks.mirrorLocalStorage).toHaveBeenCalledTimes(3);
    expect(
      mocks.mirrorLocalStorage.mock.calls.map(([entries]) => entries),
    ).toEqual([
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a"]}' },
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a","b","c"]}' },
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a","b","c","d"]}' },
    ]);
  } finally {
    window.removeEventListener("storage", mirrorLocalStorage);
  }
});

it("rejects strict flushes on write failure without poisoning later writes", async () => {
  const { flushLocalStorageMirror } = await import("./local-storage-mirror");
  mocks.mirrorLocalStorage
    .mockResolvedValueOnce({ status: "error", error: "disk full" })
    .mockResolvedValueOnce({ status: "ok", data: null });
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

  try {
    localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a"]}');
    await expect(flushLocalStorageMirror()).rejects.toThrow("disk full");

    localStorage.setItem(PENDING_KEY, '{"upserts":[],"deletes":["a","b"]}');
    await expect(flushLocalStorageMirror()).resolves.toBeUndefined();
    expect(
      mocks.mirrorLocalStorage.mock.calls.map(([entries]) => entries),
    ).toEqual([
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a"]}' },
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a","b"]}' },
    ]);
  } finally {
    warn.mockRestore();
  }
});
