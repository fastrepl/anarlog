import { Host, Icon } from "@expo/ui";
import type { ColorValue } from "react-native";

const nativeIcons = {
  back: Icon.select({
    ios: "chevron.left",
    android: import("@expo/material-symbols/arrow_back.xml"),
  }),
  close: Icon.select({
    ios: "xmark",
    android: import("@expo/material-symbols/close.xml"),
  }),
  more: Icon.select({
    ios: "ellipsis",
    android: import("@expo/material-symbols/more_horiz.xml"),
  }),
  "new-note": Icon.select({
    ios: "square.and.pencil",
    android: import("@expo/material-symbols/edit_square.xml"),
  }),
  profile: Icon.select({
    ios: "person",
    android: import("@expo/material-symbols/person.xml"),
  }),
  search: Icon.select({
    ios: "magnifyingglass",
    android: import("@expo/material-symbols/search.xml"),
  }),
} as const;

export type NativeIconName = keyof typeof nativeIcons;

export function NativeIcon({
  color,
  name,
  size,
}: {
  color: ColorValue;
  name: NativeIconName;
  size: number;
}) {
  return (
    <Host matchContents>
      <Icon color={color} name={nativeIcons[name]} size={size} />
    </Host>
  );
}
