import { Column, Icon, RNHostView } from "@expo/ui";
import { Image } from "react-native";

import {
  providerIconArtworkSize,
  providerIconSource,
} from "./provider-icon-assets";
import { useAppColorScheme } from "./theme-provider";

export function ProviderIcon({
  provider,
  size = 20,
}: {
  provider: string;
  size?: number;
}) {
  const source = providerIconSource(provider, useAppColorScheme());
  const artworkSize = providerIconArtworkSize(provider, size);
  return (
    <Column
      alignment="center"
      style={{
        width: size,
        paddingVertical: (size - artworkSize) / 2,
      }}
    >
      {source ? (
        <RNHostView matchContents>
          <Image
            source={source}
            style={{ width: artworkSize, height: artworkSize }}
            accessible={false}
          />
        </RNHostView>
      ) : (
        <Icon name="shuffle" size={artworkSize} />
      )}
    </Column>
  );
}
