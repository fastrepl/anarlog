import { vi } from "vitest";

const storage = new Map<string, string>();

Object.defineProperty(window, "localStorage", {
  configurable: true,
  writable: true,
  value: {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => {
      storage.set(key, value);
    },
    removeItem: (key: string) => {
      storage.delete(key);
    },
    clear: () => {
      storage.clear();
    },
    key: (index: number) => Array.from(storage.keys())[index] ?? null,
    get length() {
      return storage.size;
    },
  },
});

Object.defineProperty(window, "hypr", {
  configurable: true,
  writable: true,
  value: {
    db: {
      execute: vi.fn().mockResolvedValue([]),
      executeProxy: vi.fn().mockResolvedValue({ rows: [] }),
      subscribe: vi.fn().mockResolvedValue(async () => {}),
    },
    openExternal: vi.fn().mockResolvedValue(undefined),
    embeddedCli: {
      check: vi.fn().mockResolvedValue({
        supported: true,
        commandName: "char",
        installPath: "/tmp/char",
        resourcePath: null,
        state: "missing",
        details: null,
      }),
      install: vi.fn(),
      uninstall: vi.fn(),
    },
    updater: {
      check: vi.fn().mockResolvedValue(undefined),
      install: vi.fn().mockResolvedValue(undefined),
      subscribe: vi.fn(() => () => {}),
    },
  },
});

Object.defineProperty(window, "hyprPlatform", {
  configurable: true,
  writable: true,
  value: {
    os: "darwin",
  },
});
