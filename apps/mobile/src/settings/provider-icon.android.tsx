import { Icon } from "@expo/ui";
import { Image } from "@expo/ui/jetpack-compose";
import { size as iconSize } from "@expo/ui/jetpack-compose/modifiers";

import { providerIconSource } from "./provider-icon-assets";
import { useAppColorScheme, useColors } from "./theme-provider";

export function ProviderIcon({
  provider,
  size = 20,
}: {
  provider: string;
  size?: number;
}) {
  const scheme = useAppColorScheme();
  const Colors = useColors();
  const source = providerIconSource(provider, scheme);
  return source ? (
    <Image
      source={source}
      contentDescription={null}
      modifiers={[iconSize(size, size)]}
    />
  ) : (
    <Icon
      name={Icon.select({
        ios: "shuffle",
        android: import("@expo/material-symbols/shuffle.xml"),
      })}
      size={size}
      color={Colors.muted}
    />
  );
}
