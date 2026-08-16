import { Ionicons } from "@expo/vector-icons";
import type { ReactNode } from "react";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";

import { Colors, Spacing } from "@/constants/theme";
import type { EditorFormat } from "@/lib/editor-format";

function FormatButton({
  accessibilityLabel,
  format,
  onPress,
  children,
}: {
  accessibilityLabel: string;
  format: EditorFormat;
  onPress: (format: EditorFormat) => void;
  children: ReactNode;
}) {
  return (
    <Pressable
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      hitSlop={2}
      onPress={() => onPress(format)}
      style={({ pressed }) => [styles.button, pressed && styles.buttonPressed]}
    >
      {children}
    </Pressable>
  );
}

export function EditorAccessory({
  onFormat,
  onDismiss,
}: {
  onFormat: (format: EditorFormat) => void;
  onDismiss: () => void;
}) {
  return (
    <View style={styles.toolbar}>
      <ScrollView
        horizontal
        contentContainerStyle={styles.controls}
        keyboardShouldPersistTaps="always"
        showsHorizontalScrollIndicator={false}
      >
        <FormatButton
          accessibilityLabel="Heading"
          format="heading"
          onPress={onFormat}
        >
          <Text style={styles.heading}>H</Text>
        </FormatButton>
        <FormatButton
          accessibilityLabel="Bold"
          format="bold"
          onPress={onFormat}
        >
          <Text style={styles.bold}>B</Text>
        </FormatButton>
        <FormatButton
          accessibilityLabel="Italic"
          format="italic"
          onPress={onFormat}
        >
          <Text style={styles.italic}>I</Text>
        </FormatButton>
        <FormatButton
          accessibilityLabel="Bulleted list"
          format="bullet"
          onPress={onFormat}
        >
          <Ionicons name="list-outline" size={23} color={Colors.ink} />
        </FormatButton>
        <FormatButton
          accessibilityLabel="Checklist"
          format="checklist"
          onPress={onFormat}
        >
          <Ionicons name="checkbox-outline" size={22} color={Colors.ink} />
        </FormatButton>
      </ScrollView>
      <View style={styles.separator} />
      <Pressable
        accessibilityLabel="Hide keyboard"
        accessibilityRole="button"
        hitSlop={2}
        onPress={onDismiss}
        style={({ pressed }) => [
          styles.dismissButton,
          pressed && styles.buttonPressed,
        ]}
      >
        <Ionicons name="chevron-down" size={24} color={Colors.ink} />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  toolbar: {
    minHeight: 52,
    flexDirection: "row",
    alignItems: "center",
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: Colors.border,
    backgroundColor: Colors.paper,
    paddingHorizontal: Spacing.xs,
  },
  controls: {
    flexGrow: 1,
    alignItems: "center",
    gap: 2,
  },
  button: {
    width: 44,
    height: 44,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 12,
  },
  buttonPressed: {
    backgroundColor: Colors.border,
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
  separator: {
    width: StyleSheet.hairlineWidth,
    height: 30,
    backgroundColor: Colors.border,
  },
  dismissButton: {
    width: 48,
    height: 44,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: 12,
    marginLeft: Spacing.xs,
  },
});
