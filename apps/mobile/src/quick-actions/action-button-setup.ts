import AsyncStorage from "@react-native-async-storage/async-storage";
import { queryOptions, useQuery } from "@tanstack/react-query";
import * as Device from "expo-device";
import { Platform } from "react-native";

import { captureOperationalError } from "@/lib/error-reporting";
import { queryClient } from "@/lib/query-client";

import { modelHasActionButton } from "./action-button-model";

const CARD_DISMISSED_KEY = "action-button-card-dismissed";
const SHORTCUT_RAN_KEY = "action-button-shortcut-ran";

export type ActionButtonSetup = {
  cardDismissed: boolean;
  shortcutRan: boolean;
};

const initialSetup: ActionButtonSetup = {
  cardDismissed: false,
  shortcutRan: false,
};

export const deviceHasActionButton =
  Platform.OS === "ios" &&
  (!Device.isDevice || modelHasActionButton(Device.modelId));

async function loadSetup(): Promise<ActionButtonSetup> {
  try {
    const [cardDismissed, shortcutRan] = await Promise.all([
      AsyncStorage.getItem(CARD_DISMISSED_KEY),
      AsyncStorage.getItem(SHORTCUT_RAN_KEY),
    ]);
    return {
      cardDismissed: cardDismissed === "1",
      shortcutRan: shortcutRan === "1",
    };
  } catch (error) {
    captureOperationalError(error, {
      operation: "action_button_setup_load",
      level: "warning",
    });
    return initialSetup;
  }
}

export const actionButtonSetupQuery = queryOptions({
  queryKey: ["action-button-setup"],
  queryFn: loadSetup,
  staleTime: Infinity,
});

export function useActionButtonSetup() {
  return useQuery({
    ...actionButtonSetupQuery,
    enabled: deviceHasActionButton,
  });
}

async function persistFlag(
  key: string,
  patch: Partial<ActionButtonSetup>,
  operation: string,
) {
  const { queryKey } = actionButtonSetupQuery;
  await queryClient.cancelQueries({ queryKey });
  queryClient.setQueryData(queryKey, (previous) => ({
    ...(previous ?? initialSetup),
    ...patch,
  }));
  try {
    await AsyncStorage.setItem(key, "1");
  } catch (error) {
    captureOperationalError(error, { operation, level: "warning" });
  } finally {
    await queryClient.invalidateQueries({ queryKey, refetchType: "all" });
  }
}

export function dismissActionButtonCard(): Promise<void> {
  return persistFlag(
    CARD_DISMISSED_KEY,
    { cardDismissed: true },
    "action_button_card_dismiss",
  );
}

export function markStartListeningShortcutRan(): Promise<void> {
  if (!deviceHasActionButton) return Promise.resolve();
  return persistFlag(
    SHORTCUT_RAN_KEY,
    { shortcutRan: true },
    "action_button_shortcut_ran_save",
  );
}
