import { commands as detectCommands } from "@hypr/plugin-detect";

export const DISCLOSURE_VISIBLE_SECONDS = 5 * 60;
export const DEFAULT_DISCLOSURE_MESSAGE =
  "This meeting may be recorded and transcribed for notes.";

export async function sendMeetingDisclosure(appIds: string[] | null) {
  const result = await detectCommands.sendMeetingDisclosure(
    appIds,
    DEFAULT_DISCLOSURE_MESSAGE,
  );

  if (result.status === "error") {
    console.warn("[listener] failed to disclose recording", result.error);
    return false;
  }

  return true;
}
