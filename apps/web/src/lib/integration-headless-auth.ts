const HEADLESS_OAUTH_INTEGRATION_IDS = new Set([
  "google-calendar",
  "outlook",
  "linear",
  "github",
  "slack",
  "notion",
  "zoom",
  "fathom",
  "webex",
  "google-meet",
  "microsoft-teams",
]);

export function usesHeadlessOAuth(integrationId: string) {
  return HEADLESS_OAUTH_INTEGRATION_IDS.has(integrationId);
}

export function isConnectSessionFailed({
  handedOffToken,
  isError,
  token,
}: {
  handedOffToken?: string;
  isError: boolean;
  token?: string;
}) {
  return !handedOffToken && isError && !token;
}
