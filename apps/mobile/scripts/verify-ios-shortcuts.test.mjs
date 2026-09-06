import assert from "node:assert/strict";
import { test } from "node:test";

import { verifyAppShortcuts } from "./verify-ios-shortcuts.mjs";

const actions = { ToggleListeningIntent: { openAppWhenRun: true } };
const autoShortcuts = [
  {
    actionIdentifier: "ToggleListeningIntent",
    shortTitle: { key: "Start Listening" },
  },
];

test("rejects a build that extracts the intent but drops its App Shortcut", () => {
  assert.throws(() => verifyAppShortcuts({ actions }), /ready-made/);
});

test("accepts an app with the ready-made recording shortcut", () => {
  verifyAppShortcuts({ actions, autoShortcuts });
});

test("rejects a shortcut that cannot foreground microphone capture", () => {
  assert.throws(
    () => verifyAppShortcuts({ actions: {}, autoShortcuts }),
    /open Anarlog/,
  );
});
