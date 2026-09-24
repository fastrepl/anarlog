import AsyncStorage from "@react-native-async-storage/async-storage";
import { useQuery } from "@tanstack/react-query";
import * as Device from "expo-device";
import { Platform } from "react-native";

import { captureOperationalError } from "@/lib/error-reporting";
import { queryClient } from "@/lib/query-client";

import { modelHasActionButton } from "./action-button-model";

const CARD_DISMISSED_KEY = "action-button-card-dismissed";
const VERIFIED_KEY = "action-button-verified";
const setupQueryKey = ["action-button-setup"] as const;

export type ActionButtonSetup = {
  cardDismissed: boolean;
  verified: boolean;
};

const initialSetup: ActionButtonSetup = {
  cardDismissed: false,
  verified: false,
};

export const deviceHasActionButton =
  Platform.OS === "ios" &&
  (!Device.isDevice || modelHasActionButton(Device.modelId));

async function loadSetup(): Promise<ActionButtonSetup> {
  try {
    const [cardDismissed, verified] = await Promise.all([
      AsyncStorage.getItem(CARD_DISMISSED_KEY),
      AsyncStorage.getItem(VERIFIED_KEY),
    ]);
    return { cardDismissed: cardDismissed === "1", verified: verified === "1" };
  } catch (error) {
    captureOperationalError(error, {
      operation: "action_button_setup_load",
      level: "warning",
    });
    return initialSetup;
  }
}

export function useActionButtonSetup() {
  return useQuery({
    queryKey: setupQueryKey,
    queryFn: loadSetup,
    enabled: deviceHasActionButton,
    staleTime: Infinity,
  });
}

async function persistFlag(
  key: string,
  patch: Partial<ActionButtonSetup>,
  operation: string,
) {
  queryClient.setQueryData<ActionButtonSetup>(setupQueryKey, (previous) => ({
    ...(previous ?? initialSetup),
    ...patch,
  }));
  try {
    await AsyncStorage.setItem(key, "1");
  } catch (error) {
    captureOperationalError(error, { operation, level: "warning" });
  }
}

export function dismissActionButtonCard(): Promise<void> {
  return persistFlag(
    CARD_DISMISSED_KEY,
    { cardDismissed: true },
    "action_button_card_dismiss",
  );
}

export function markActionButtonVerified(): Promise<void> {
  if (!deviceHasActionButton) return Promise.resolve();
  return persistFlag(
    VERIFIED_KEY,
    { verified: true },
    "action_button_verified_save",
  );
}
