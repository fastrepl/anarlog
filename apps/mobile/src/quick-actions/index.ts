import { Platform } from "react-native";

import QuickActionsModule, {
  type QuickAction,
} from "../../modules/quick-actions";

export function consumePendingQuickAction(): QuickAction | null {
  if (Platform.OS !== "ios" || !QuickActionsModule) return null;
  return QuickActionsModule.consumePendingAction();
}

export function subscribeQuickActions(listener: () => void): () => void {
  if (Platform.OS !== "ios" || !QuickActionsModule) return () => {};
  const subscription = QuickActionsModule.addListener("onAction", listener);
  return () => subscription.remove();
}
