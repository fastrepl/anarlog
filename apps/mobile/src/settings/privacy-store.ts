import AsyncStorage from "@react-native-async-storage/async-storage";
import { useSyncExternalStore } from "react";

const STORAGE_KEY = "anarlog:device-privacy";
let snapshot = {
  ready: false,
  analytics: false,
  errorReports: false,
  appLock: false,
};
const listeners = new Set<() => void>();
let loading: Promise<void> | undefined;

export function getPrivacyPreferences() {
  return snapshot;
}
export function subscribePrivacyPreferences(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export function usePrivacyPreferences() {
  return useSyncExternalStore(
    subscribePrivacyPreferences,
    getPrivacyPreferences,
    getPrivacyPreferences,
  );
}
function publish(value: typeof snapshot) {
  snapshot = value;
  listeners.forEach((listener) => listener());
}
export function preferencesFromStoredValue(stored: string | null) {
  try {
    const value: unknown = stored ? JSON.parse(stored) : {};
    const preferences =
      value && typeof value === "object"
        ? (value as Record<string, unknown>)
        : {};
    return {
      analytics: preferences.analytics !== false,
      errorReports: preferences.errorReports !== false,
      appLock: preferences.appLock === true,
      corrupt: false,
    };
  } catch {
    return {
      analytics: true,
      errorReports: true,
      appLock: false,
      corrupt: true,
    };
  }
}

export function loadPrivacyPreferences(): Promise<void> {
  loading ??= (async () => {
    let stored: string | null = null;
    try {
      stored = await AsyncStorage.getItem(STORAGE_KEY);
    } catch {
      stored = null;
    }
    const { corrupt, ...preferences } = preferencesFromStoredValue(stored);
    if (corrupt) {
      try {
        await AsyncStorage.removeItem(STORAGE_KEY);
      } catch {
        // Notes must still open if cleanup fails.
      }
    }
    publish({ ready: true, ...preferences });
  })().catch((error) => {
    loading = undefined;
    publish({
      ready: true,
      analytics: true,
      errorReports: true,
      appLock: false,
    });
    throw error;
  });
  return loading;
}
let saving = Promise.resolve();
export function setPrivacyPreference(
  key: "analytics" | "errorReports" | "appLock",
  value: boolean,
): Promise<void> {
  const operation = saving
    .catch(() => {})
    .then(async () => {
      await loadPrivacyPreferences();
      const next = { ...snapshot, [key]: value };
      await AsyncStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      publish(next);
    });
  saving = operation;
  return operation;
}
