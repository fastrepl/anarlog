import { randomUUID } from "node:crypto";
import * as React from "react";
import { vi } from "vitest";

Object.defineProperty(globalThis.crypto, "randomUUID", { value: randomUUID });

if (typeof globalThis.window !== "undefined") {
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
      transformCallback: vi.fn((callback: unknown) => {
        const callbackId = Math.trunc(Math.random() * Number.MAX_SAFE_INTEGER);
        Object.assign(globalThis.window, {
          [`_${callbackId}`]: callback,
        });

        return callbackId;
      }),
      unregisterCallback: vi.fn((callbackId: number) => {
        delete (globalThis.window as unknown as Record<string, unknown>)[
          `_${callbackId}`
        ];
      }),
      invoke: vi.fn((command: string) =>
        Promise.resolve(command === "plugin:event|listen" ? 0 : null),
      ),
    },
    writable: true,
    configurable: true,
  });

  Object.defineProperty(globalThis.window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
    value: {
      unregisterListener: vi.fn(),
    },
    writable: true,
    configurable: true,
  });
}

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn((path: string) =>
    Promise.resolve(`/resources/${path}`),
  ),
  sep: vi.fn().mockReturnValue("/"),
}));

vi.mock("@hypr/plugin-db", () => ({
  bindCloudsyncAccount: vi.fn().mockResolvedValue(true),
  configureCloudsyncToken: vi.fn().mockResolvedValue("configured"),
  execute: vi.fn().mockResolvedValue([]),
  executeProxy: vi.fn().mockResolvedValue({ rows: [] }),
  executeTransaction: vi.fn().mockResolvedValue([]),
  getE2eeIdentityStatus: vi.fn().mockResolvedValue({
    configured: true,
    keyId: "abcdefghijklmnopqrstuv",
  }),
  createE2eeIdentity: vi.fn(),
  inspectE2eeRecoveryKey: vi
    .fn()
    .mockResolvedValue({ keyId: "abcdefghijklmnopqrstuv" }),
  importE2eeIdentity: vi.fn(),
  getCloudsyncStatus: vi.fn().mockResolvedValue({
    cloudsync_enabled: true,
    extension_loaded: true,
    configured: false,
    running: false,
    network_initialized: false,
    last_sync: null,
    last_sync_at_ms: null,
    has_unsent_changes: null,
    last_error: null,
    last_error_kind: null,
    consecutive_failures: 0,
  }),
  getMeeting: vi.fn(),
  getMeetingTranscript: vi.fn(),
  getRecurringMeetingHistory: vi.fn(),
  listMeetings: vi.fn(),
  subscribe: vi.fn().mockResolvedValue(() => Promise.resolve()),
  suspendCloudsync: vi.fn().mockResolvedValue(undefined),
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
    i18n: { _: translate, locale: "en" },
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
    showDevtool: vi.fn().mockResolvedValue(true),
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
