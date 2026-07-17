import { commands as windowsCommands } from "@hypr/plugin-windows";

import {
  beginCanonicalSessionEditorActivation,
  waitForCanonicalSessionImportUnlock,
} from "~/session-sharing/editor-activity";

export async function openStandaloneNoteWindow(sessionId: string) {
  let finishActivation = beginCanonicalSessionEditorActivation(sessionId);
  while (!finishActivation) {
    await waitForCanonicalSessionImportUnlock(sessionId);
    finishActivation = beginCanonicalSessionEditorActivation(sessionId);
  }

  const result = await windowsCommands
    .windowShow({
      type: "note",
      value: sessionId,
    })
    .finally(finishActivation);

  if (result.status === "error") {
    console.error("Failed to open note window:", result.error);
  }
}
