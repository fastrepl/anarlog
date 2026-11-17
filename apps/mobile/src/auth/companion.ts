import { Platform } from "react-native";

// On iOS, Anarlog is a free companion to an existing Anarlog Pro account
// (App Store Guideline 3.1.3(f)): it requires Pro, starts no trial, and shows
// no purchase or subscription prompts.
export const requiresPro = Platform.OS === "ios";

// The web account page hides plan offers when the iOS app opens it.
export function accountPageUrl(appUrl: string, section?: "danger") {
  const url = new URL(`${appUrl.replace(/\/+$/, "")}/app/account`);
  if (requiresPro) url.searchParams.set("client", "ios");
  if (section) url.hash = section;
  return url.toString();
}
