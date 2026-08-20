export const PROVIDER_BRAND_ICONS: Record<string, string> = {
  "chatgpt-record": "/assets/model-icons/openai-logo.svg",
  "google-meet": "/assets/google-meet.svg",
  "microsoft-teams": "/assets/microsoft-teams.svg",
  "slack-huddles": "/assets/slack-icon.svg",
  zoom: "/assets/zoom-icon.svg",
};

// Meet has no desktop app icon. Zoom's native icon is the wordmark, not the camera.
const BRAND_ICON_OVERRIDES = new Set(["google-meet", "zoom"]);

export function providerIconSrc(provider: {
  id: string;
  iconUrl?: string;
}): string | undefined {
  const brandIcon = PROVIDER_BRAND_ICONS[provider.id];
  if (
    brandIcon &&
    (BRAND_ICON_OVERRIDES.has(provider.id) || !provider.iconUrl)
  ) {
    return brandIcon;
  }
  return provider.iconUrl ?? brandIcon;
}
