import {
  useCssElement,
  useNativeVariable as useFunctionalVariable,
  type StyledConfiguration,
  type StyledProps,
} from "react-native-css";

import Animated from "react-native-reanimated";
import React from "react";
import {
  type PressableProps as RNPressableProps,
  type ScrollViewProps as RNScrollViewProps,
  type TextInputProps as RNTextInputProps,
  type TextProps as RNTextProps,
  type ViewProps as RNViewProps,
  View as RNView,
  Text as RNText,
  Pressable as RNPressable,
  ScrollView as RNScrollView,
  TextInput as RNTextInput,
} from "react-native";

const viewMapping = {
  className: "style",
} satisfies StyledConfiguration<typeof RNView>;

const textMapping = {
  className: "style",
} satisfies StyledConfiguration<typeof RNText>;

const scrollViewMapping = {
  className: "style",
  contentContainerClassName: "contentContainerStyle",
} as const;

const pressableMapping = {
  className: "style",
} as const;

const textInputMapping = {
  className: "style",
} satisfies StyledConfiguration<typeof RNTextInput>;

const animatedScrollViewMapping = {
  className: "style",
  contentContainerClassName: "contentContainerStyle",
} as const;

export const useCSSVariable =
  process.env.EXPO_OS !== "web"
    ? useFunctionalVariable
    : (variable: string) => `var(${variable})`;

export type ViewProps = StyledProps<RNViewProps, typeof viewMapping>;

export const View = (props: ViewProps) => {
  return useCssElement(RNView, props, viewMapping);
};
View.displayName = "CSS(View)";

type TextProps = StyledProps<RNTextProps, typeof textMapping>;

export const Text = (props: TextProps) => {
  return useCssElement(RNText, props, textMapping);
};
Text.displayName = "CSS(Text)";

type ScrollViewProps = RNScrollViewProps & {
  className?: string;
  contentContainerClassName?: string;
};

export const ScrollView = (props: ScrollViewProps) => {
  return useCssElement(RNScrollView as any, props, scrollViewMapping as any);
};
ScrollView.displayName = "CSS(ScrollView)";

type PressableProps = RNPressableProps & { className?: string };

export const Pressable = (props: PressableProps) => {
  return useCssElement(RNPressable as any, props, pressableMapping as any);
};
Pressable.displayName = "CSS(Pressable)";

type TextInputProps = StyledProps<RNTextInputProps, typeof textInputMapping>;

export const TextInput = (props: TextInputProps) => {
  return useCssElement(RNTextInput, props, textInputMapping);
};
TextInput.displayName = "CSS(TextInput)";

type AnimatedScrollViewProps = React.ComponentProps<typeof Animated.ScrollView> &
  {
    className?: string;
    contentContainerClassName?: string;
  };

export const AnimatedScrollView = (props: AnimatedScrollViewProps) => {
  return useCssElement(
    Animated.ScrollView as any,
    props,
    animatedScrollViewMapping as any,
  );
};
AnimatedScrollView.displayName = "CSS(AnimatedScrollView)";
