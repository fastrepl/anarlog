import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

export function verifyAppShortcuts(metadata) {
  assert(
    metadata.actions?.ToggleListeningIntent?.openAppWhenRun,
    "The recording intent must open Anarlog to start microphone capture.",
  );
  assert(
    metadata.autoShortcuts?.some(
      (shortcut) =>
        shortcut.actionIdentifier === "ToggleListeningIntent" &&
        shortcut.shortTitle?.key === "Start Listening",
    ),
    "The app must include the ready-made Start Listening shortcut, not only its intent.",
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  assert(process.argv[2], "Pass the built .app directory.");
  const metadata = JSON.parse(
    readFileSync(
      path.join(process.argv[2], "Metadata.appintents", "extract.actionsdata"),
      "utf8",
    ),
  );
  verifyAppShortcuts(metadata);
  console.log("Start Listening App Shortcut is packaged in the app.");
}
