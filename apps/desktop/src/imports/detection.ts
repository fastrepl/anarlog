import { commands as detectCommands } from "@anlg/plugin-detect";

import { detectMeetingImportProviders } from "./providers";

export async function detectImportSources() {
  const installedResult = await detectCommands.listInstalledApplications();
  if (installedResult.status === "error") {
    throw new Error(installedResult.error);
  }

  const providers = detectMeetingImportProviders(installedResult.data);
  if (providers.length === 0) return providers;

  try {
    const iconsResult = await detectCommands.getInstalledApplicationIcons(
      providers.map((provider) => provider.installedAppId),
    );
    if (iconsResult.status === "error") return providers;

    const icons = new Map(
      iconsResult.data.map((icon) => [icon.id, icon.dataUrl]),
    );
    return providers.map((provider) => ({
      ...provider,
      iconUrl: icons.get(provider.installedAppId),
    }));
  } catch {
    return providers;
  }
}
