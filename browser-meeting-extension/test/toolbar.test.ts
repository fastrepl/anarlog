import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import vm from "node:vm";

import { expect, it, vi } from "vitest";

const read = (name: string) =>
  readFileSync(resolve(import.meta.dirname, "../extension", name), "utf8");

it("asks for no Meet host access: a Meet tab is reachable only through the click's activeTab", () => {
  const manifest = JSON.parse(read("manifest.json"));
  expect(manifest.permissions.sort()).toEqual([
    "activeTab",
    "contextMenus",
    "nativeMessaging",
    "scripting",
  ]);
  expect(manifest).not.toHaveProperty("host_permissions");
  expect(manifest).not.toHaveProperty("optional_host_permissions");
  expect(manifest).not.toHaveProperty("content_scripts");
  expect(manifest).not.toHaveProperty("externally_connectable");
});

it("hands a clicked Meet call to the recorder and tells other pages to open a call", () => {
  const listeners: Record<string, (...args: any[]) => unknown> = {};
  const event = (name: string) => ({
    addListener: (fn: (...args: any[]) => unknown) => void (listeners[name] = fn),
  });
  const connectNative = vi.fn(() => ({
    postMessage: () => {},
    disconnect: () => {},
    onMessage: { addListener: () => {} },
    onDisconnect: { addListener: () => {} },
  }));
  const title = vi.fn();
  const chrome = {
    action: { setBadgeText: () => {}, setTitle: title, onClicked: event("click") },
    runtime: {
      connectNative,
      onInstalled: event("install"),
      onStartup: event("startup"),
      onMessage: event("message"),
    },
    contextMenus: { create: () => {}, removeAll: async () => {}, onClicked: event("menu") },
    tabs: { onUpdated: event("updated"), onRemoved: event("removed") },
  };
  const worker = vm.createContext({ chrome, URL, setTimeout, clearTimeout });
  for (const script of ["config.js", "meeting-background.js", "toolbar.js"])
    vm.runInContext(read(script), worker);

  listeners.click({ id: 3, url: "https://docs.google.com/document/d/1" });
  expect(connectNative).not.toHaveBeenCalled();
  expect(title).toHaveBeenLastCalledWith({
    tabId: 3,
    title: "Open a Google Meet call, then click Anarlog to record it.",
  });
  listeners.click({ id: 4, url: "https://meet.google.com/abc-defg-hij" });
  expect(connectNative).toHaveBeenCalledWith("so.anarlog.browser_meeting");
});
