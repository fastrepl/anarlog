import { requireNativeView } from "expo";
import type { ViewProps } from "react-native";

export const AnarlogShortcutsButton = requireNativeView<ViewProps>(
  "AnarlogQuickActions",
);
