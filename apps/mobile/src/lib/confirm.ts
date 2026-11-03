import { Alert, Platform } from "react-native";

import { getAppLanguage, translate } from "@/i18n";

// RN-web's Alert is a no-op, so web falls back to window.confirm.
export function confirmDestructive(
  title: string,
  action: string,
): Promise<boolean> {
  if (Platform.OS === "web") {
    return Promise.resolve(
      typeof window !== "undefined" &&
        window.confirm(translate(title, getAppLanguage())),
    );
  }
  return new Promise((resolve) => {
    Alert.alert(
      translate(title, getAppLanguage()),
      undefined,
      [
        {
          text: translate("Cancel", getAppLanguage()),
          style: "cancel",
          onPress: () => resolve(false),
        },
        {
          text: translate(action, getAppLanguage()),
          style: "destructive",
          onPress: () => resolve(true),
        },
      ],
      { cancelable: true, onDismiss: () => resolve(false) },
    );
  });
}
