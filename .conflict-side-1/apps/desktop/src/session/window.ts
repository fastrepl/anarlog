import { commands as windowsCommands } from "@hypr/plugin-windows";

export async function openStandaloneNoteWindow(sessionId: string) {
  const result = await windowsCommands.windowShow({
    type: "note",
    value: sessionId,
  });

  if (result.status === "error") {
    console.error("Failed to open note window:", result.error);
  }
}
