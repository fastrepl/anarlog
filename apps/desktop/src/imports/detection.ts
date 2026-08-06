import { commands as detectCommands } from "@anlg/plugin-detect";

import { detectMeetingImportProviders } from "./providers";

export async function detectImportSources() {
  const installedResult = await detectCommands.listInstalledApplications();
  if (installedResult.status === "error") {
    throw new Error(installedResult.error);
  }

  return detectMeetingImportProviders(installedResult.data);
}
