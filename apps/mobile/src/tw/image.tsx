import {
  useCssElement,
  type StyledProps,
} from "react-native-css";
import React from "react";
import { StyleSheet } from "react-native";
import Animated from "react-native-reanimated";
import { Image as RNImage } from "expo-image";

const AnimatedExpoImage = Animated.createAnimatedComponent(RNImage);
const imageMapping = {
  className: "style",
} as const;
type CSSImageProps = React.ComponentProps<typeof AnimatedExpoImage>;
type ImageProps = CSSImageProps & { className?: string };

function CSSImage(props: CSSImageProps) {
  // @ts-expect-error: Remap objectFit style to contentFit property
  const { objectFit, objectPosition, ...style } =
    StyleSheet.flatten(props.style) || {};

  return (
    <AnimatedExpoImage
      contentFit={objectFit}
      contentPosition={objectPosition}
      {...props}
      source={
        typeof props.source === "string" ? { uri: props.source } : props.source
      }
      // @ts-expect-error: Style is remapped above
      style={style}
    />
  );
}

export const Image = (props: ImageProps) => {
  return useCssElement(CSSImage as any, props, imageMapping as any);
};
Image.displayName = "CSS(Image)";
