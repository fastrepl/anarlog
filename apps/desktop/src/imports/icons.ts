export const PROVIDER_BRAND_ICONS: Record<string, string> = {
  "chatgpt-record": "/assets/model-icons/openai-logo.svg",
  "google-meet": "/assets/google-meet.svg",
  "microsoft-teams": "/assets/microsoft-teams.svg",
  "slack-huddles": "/assets/slack-icon.svg",
  zoom: "/assets/zoom-icon.svg",
};

// Meet has no desktop app icon. Zoom's native icon is the wordmark, not the camera.
const BRAND_ICON_OVERRIDES = new Set(["google-meet", "zoom"]);

// Line-art marks read smaller than filled app icons in the same 32px slot.
const BRAND_ICON_OPTICAL_SCALE: Record<string, number> = {
  "chatgpt-record": 1.22,
  "slack-huddles": 1.12,
};

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

export function providerIconOpticalScale(provider: {
  id: string;
  iconUrl?: string;
}): number | undefined {
  const src = providerIconSrc(provider);
  if (src !== PROVIDER_BRAND_ICONS[provider.id]) return undefined;
  return BRAND_ICON_OPTICAL_SCALE[provider.id];
}
