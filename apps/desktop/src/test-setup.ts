import { randomUUID } from "node:crypto";
import * as React from "react";
import { vi } from "vitest";

Object.defineProperty(globalThis.crypto, "randomUUID", { value: randomUUID });

Object.defineProperty(globalThis.window, "__TAURI_INTERNALS__", {
  value: {
    metadata: {
      currentWindow: {
        label: "main",
      },
      currentWebview: {
        label: "main",
      },
    },
    invoke: vi.fn().mockRejectedValue(new Error("not available in test")),
  },
  writable: true,
  configurable: true,
});

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@hypr/plugin-db", () => ({
  execute: vi.fn().mockResolvedValue([]),
  executeProxy: vi.fn().mockResolvedValue({ rows: [] }),
  subscribe: vi.fn().mockResolvedValue(() => {}),
}));

function translate(
  input:
    | TemplateStringsArray
    | string
    | { message?: string; values?: Record<string, unknown> },
  ...values: unknown[]
) {
  if (typeof input === "string") {
    return input;
  }

  if (typeof input === "object" && !("raw" in input)) {
    let message = input.message ?? "";
    for (const [key, value] of Object.entries(input.values ?? {})) {
      message = message.split(`{${key}}`).join(String(value));
    }
    return message;
  }

  return Array.from(input).reduce(
    (text, part, index) => `${text}${part}${values[index] ?? ""}`,
    "",
  );
}

vi.mock("@lingui/react/macro", () => ({
  Trans: ({
    children,
    id,
    message,
    values,
  }: {
    children?: React.ReactNode;
    id?: string;
    message?: string;
    values?: Record<string, unknown>;
  }) =>
    React.createElement(
      React.Fragment,
      null,
      children ?? translate({ message: message ?? id, values }),
    ),
  useLingui: () => ({
    _: translate,
    t: translate,
  }),
}));

vi.mock("@lingui/react", () => ({
  I18nProvider: ({ children }: { children?: React.ReactNode }) =>
    React.createElement(React.Fragment, null, children),
  Trans: ({
    children,
    id,
    message,
    values,
  }: {
    children?: React.ReactNode;
    id?: string;
    message?: string;
    values?: Record<string, unknown>;
  }) =>
    React.createElement(
      React.Fragment,
      null,
      children ?? translate({ message: message ?? id, values }),
    ),
  useLingui: () => ({
    _: translate,
    t: translate,
    i18n: { locale: "en" },
  }),
}));

vi.mock("@hypr/plugin-analytics", () => ({
  commands: {
    event: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setProperties: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setDisabled: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    isDisabled: vi.fn().mockResolvedValue({ status: "ok", data: false }),
  },
}));

vi.mock("./types/tauri.gen", () => ({
  commands: {
    getOnboardingNeeded: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: false }),
    getPinnedTabs: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    setPinnedTabs: vi.fn().mockResolvedValue({ status: "ok", data: null }),
    getRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
    setRecentlyOpenedSessions: vi
      .fn()
      .mockResolvedValue({ status: "ok", data: null }),
  },
}));
