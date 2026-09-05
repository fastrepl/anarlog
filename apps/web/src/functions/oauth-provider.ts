export type OAuthProvider = "apple" | "azure" | "google" | "github";

export function oauthProviderScopes(provider: OAuthProvider) {
  if (provider === "azure") {
    return "openid email profile";
  }
  return undefined;
}
