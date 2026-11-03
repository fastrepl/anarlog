import {
  Button as ExpoButton,
  FieldGroup,
  ListItem as ExpoListItem,
  Switch as ExpoSwitch,
  Text as ExpoText,
  TextInput as ExpoTextInput,
} from "@expo/ui";
import type { ComponentProps } from "react";

import { translate, useLanguage } from "@/i18n";

export { Picker } from "@expo/ui";

export function Button(props: ComponentProps<typeof ExpoButton>) {
  const { language } = useLanguage();
  return (
    <ExpoButton
      {...props}
      label={props.label ? translate(props.label, language) : props.label}
    />
  );
}

export function Switch(props: ComponentProps<typeof ExpoSwitch>) {
  const { language } = useLanguage();
  return (
    <ExpoSwitch
      {...props}
      label={props.label ? translate(props.label, language) : props.label}
    />
  );
}

export function ListItem(props: ComponentProps<typeof ExpoListItem>) {
  const { language } = useLanguage();
  return (
    <ExpoListItem
      {...props}
      supportingText={
        typeof props.supportingText === "string"
          ? translate(props.supportingText, language)
          : props.supportingText
      }
    >
      {typeof props.children === "string"
        ? translate(props.children, language)
        : props.children}
    </ExpoListItem>
  );
}

export function TextInput(props: ComponentProps<typeof ExpoTextInput>) {
  const { language } = useLanguage();
  return (
    <ExpoTextInput
      {...props}
      placeholder={
        typeof props.placeholder === "string"
          ? translate(props.placeholder, language)
          : props.placeholder
      }
    />
  );
}

export function Text(props: ComponentProps<typeof ExpoText>) {
  const { language } = useLanguage();
  const children =
    typeof props.children === "string"
      ? translate(props.children, language)
      : props.children;
  return <ExpoText {...props}>{children}</ExpoText>;
}

export const FieldFooter = FieldGroup.SectionFooter;
