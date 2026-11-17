import { Platform } from "react-native";

// On iOS, Anarlog is a free companion to an existing Anarlog Pro account
// (App Store Guideline 3.1.3(f)): it requires Pro, starts no trial, and shows
// no purchase or subscription prompts.
export const requiresPro = Platform.OS === "ios";
