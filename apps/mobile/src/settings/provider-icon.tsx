import { Icon, RNHostView } from "@expo/ui";
import { Image } from "react-native";

import { providerIconSource } from "./provider-icon-assets";
import { useAppColorScheme } from "./theme-provider";

export function ProviderIcon({
  provider,
  size = 20,
}: {
  provider: string;
  size?: number;
}) {
  const source = providerIconSource(provider, useAppColorScheme());
  return source ? (
    <RNHostView matchContents>
      <Image
        source={source}
        style={{ width: size, height: size }}
        accessible={false}
      />
    </RNHostView>
  ) : (
    <Icon name="shuffle" size={size} />
  );
}
