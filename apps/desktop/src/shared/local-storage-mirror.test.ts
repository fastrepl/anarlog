import { afterEach, beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  mirrorLocalStorage:
    vi.fn<(entries: Record<string, string>) => Promise<unknown>>(),
}));

vi.mock("~/types/tauri.gen", () => ({
  commands: { mirrorLocalStorage: mocks.mirrorLocalStorage },
}));

const PENDING_KEY = "anarlog.cloud-api-pending.v1.user-1";

beforeEach(() => {
  vi.resetModules();
  vi.useFakeTimers();
  localStorage.clear();
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

it("mirrors immediately at startup while still debouncing later changes", async () => {
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
    expect(
      mocks.mirrorLocalStorage.mock.calls.map(([entries]) => entries),
    ).toEqual([
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a"]}' },
      { [PENDING_KEY]: '{"upserts":[],"deletes":["a","b","c"]}' },
    ]);
  } finally {
    window.removeEventListener("storage", mirrorLocalStorage);
  }
});
