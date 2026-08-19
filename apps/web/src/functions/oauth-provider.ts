export type OAuthProvider = "azure" | "google" | "github";

export function oauthProviderScopes(provider: OAuthProvider, rra?: boolean) {
  if (provider === "github" && rra) {
    return "repo";
  }
  if (provider === "azure") {
    return "openid email profile";
  }
  return undefined;
}
