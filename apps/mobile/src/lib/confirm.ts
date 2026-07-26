import { Alert, Platform } from "react-native";

// RN-web's Alert is a no-op, so web falls back to window.confirm.
export function confirmDestructive(
  title: string,
  action: string,
): Promise<boolean> {
  if (Platform.OS === "web") {
    return Promise.resolve(
      typeof window !== "undefined" && window.confirm(title),
    );
  }
  return new Promise((resolve) => {
    Alert.alert(
      title,
      undefined,
      [
        { text: "Cancel", style: "cancel", onPress: () => resolve(false) },
        { text: action, style: "destructive", onPress: () => resolve(true) },
      ],
      { cancelable: true, onDismiss: () => resolve(false) },
    );
  });
}
