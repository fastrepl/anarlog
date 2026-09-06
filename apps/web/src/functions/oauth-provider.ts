export type OAuthProvider = "apple" | "azure" | "google" | "github";

export function oauthProviderQueryParams(provider: OAuthProvider) {
  if (provider === "google" || provider === "azure") {
    return { prompt: "select_account" };
  }
  return undefined;
}

export function oauthProviderScopes(provider: OAuthProvider) {
  if (provider === "azure") {
    return "openid email profile";
  }
  return undefined;
}
