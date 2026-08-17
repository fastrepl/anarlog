import { Ionicons } from "@expo/vector-icons";
import { SymbolView } from "expo-symbols";
import type { ReactNode } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import {
  Colors,
  ControlSize,
  CornerCurve,
  Radius,
  Spacing,
} from "@/constants/theme";
import type { EditorFormat } from "@/lib/editor-format";

function FormatButton({
  accessibilityLabel,
  disabled,
  format,
  onPress,
  children,
}: {
  accessibilityLabel: string;
  disabled: boolean;
  format: EditorFormat;
  onPress: (format: EditorFormat) => void;
  children: ReactNode;
}) {
  return (
    <Pressable
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      accessibilityState={{ disabled }}
      disabled={disabled}
      hitSlop={2}
      onPress={() => onPress(format)}
      style={({ pressed }) => [
        styles.button,
        disabled && styles.buttonDisabled,
        pressed && styles.buttonPressed,
      ]}
    >
      {children}
    </Pressable>
  );
}

export function EditorAccessory({
  attaching,
  onAttach,
  onFormat,
  onDismiss,
}: {
  attaching: boolean;
  onAttach: () => void;
  onFormat: (format: EditorFormat) => void;
  onDismiss: () => void;
}) {
  return (
    <View style={styles.toolbar}>
      <View style={[styles.formatting, styles.floatingControl]}>
        <ScrollView
          horizontal
          style={styles.formattingScroll}
          contentContainerStyle={styles.controls}
          keyboardShouldPersistTaps="always"
          showsHorizontalScrollIndicator={false}
        >
          <Pressable
            accessibilityLabel="Attach file"
            accessibilityRole="button"
            disabled={attaching}
            hitSlop={2}
            onPress={onAttach}
            style={({ pressed }) => [
              styles.button,
              attaching && styles.buttonDisabled,
              pressed && styles.buttonPressed,
            ]}
          >
            <Ionicons name="attach-outline" size={24} color={Colors.ink} />
          </Pressable>
          <FormatButton
            accessibilityLabel="Heading"
            disabled={attaching}
            format="heading"
            onPress={onFormat}
          >
            <Text style={styles.heading}>H</Text>
          </FormatButton>
          <FormatButton
            accessibilityLabel="Bold"
            disabled={attaching}
            format="bold"
            onPress={onFormat}
          >
            <Text style={styles.bold}>B</Text>
          </FormatButton>
          <FormatButton
            accessibilityLabel="Italic"
            disabled={attaching}
            format="italic"
            onPress={onFormat}
          >
            <Text style={styles.italic}>I</Text>
          </FormatButton>
          <FormatButton
            accessibilityLabel="Bulleted list"
            disabled={attaching}
            format="bullet"
            onPress={onFormat}
          >
            <Ionicons name="list-outline" size={23} color={Colors.ink} />
          </FormatButton>
          <FormatButton
            accessibilityLabel="Checklist"
            disabled={attaching}
            format="checklist"
            onPress={onFormat}
          >
            <Ionicons name="checkbox-outline" size={22} color={Colors.ink} />
          </FormatButton>
        </ScrollView>
      </View>
      <Pressable
        accessibilityLabel="Hide keyboard"
        accessibilityRole="button"
        hitSlop={2}
        onPress={onDismiss}
        style={({ pressed }) => [
          styles.dismissButton,
          styles.floatingControl,
          pressed && styles.buttonPressed,
        ]}
      >
        <SymbolView
          name={{
            ios: "keyboard.chevron.compact.down",
            android: "keyboard_hide",
            web: "keyboard_hide",
          }}
          size={26}
          tintColor={Colors.ink}
          fallback={
            <Ionicons name="chevron-down" size={24} color={Colors.ink} />
          }
        />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  toolbar: {
    minHeight: ControlSize.editorAccessory + Spacing.md,
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.sm,
    backgroundColor: Colors.background,
    paddingHorizontal: Spacing.sm,
    paddingVertical: Spacing.xs,
  },
  formatting: {
    flex: 1,
    height: ControlSize.editorAccessory,
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
  },
  formattingScroll: {
    borderRadius: Radius.pill,
  },
  controls: {
    flexGrow: 1,
    alignItems: "center",
    justifyContent: "space-evenly",
    paddingHorizontal: Spacing.xs,
  },
  button: {
    width: 44,
    height: 44,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.control,
    borderCurve: CornerCurve.squircle,
  },
  buttonPressed: {
    backgroundColor: Colors.accentSurface,
  },
  buttonDisabled: {
    opacity: 0.45,
  },
  heading: {
    fontSize: 20,
    fontWeight: "700",
    color: Colors.ink,
  },
  bold: {
    fontSize: 21,
    fontWeight: "800",
    color: Colors.ink,
  },
  italic: {
    fontSize: 21,
    fontStyle: "italic",
    fontWeight: "600",
    color: Colors.ink,
  },
  floatingControl: {
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    shadowColor: Colors.ink,
    shadowOffset: { width: 0, height: 2 },
    shadowOpacity: 0.1,
    shadowRadius: 8,
    elevation: 4,
  },
  dismissButton: {
    width: ControlSize.editorAccessory,
    height: ControlSize.editorAccessory,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.pill,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.surface,
  },
});
