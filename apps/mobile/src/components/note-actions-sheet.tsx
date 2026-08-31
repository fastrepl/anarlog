import { Ionicons } from "@expo/vector-icons";
import { Modal, Pressable, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import {
  Colors,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";

export function NoteActionsSheet({
  hasRecordingHistory,
  listening,
  onClose,
  onDelete,
  onExport,
  onImportRecording,
  onToggleListening,
  visible,
}: {
  hasRecordingHistory: boolean;
  listening: boolean;
  onClose: () => void;
  onDelete: () => void;
  onExport: () => void;
  onImportRecording: () => void;
  onToggleListening: () => void;
  visible: boolean;
}) {
  const insets = useSafeAreaInsets();
  const listeningLabel = listening ? "Stop listening" : "Start listening";

  const select = (action: () => void) => {
    onClose();
    requestAnimationFrame(action);
  };

  return (
    <Modal
      animationType="slide"
      onRequestClose={onClose}
      presentationStyle="overFullScreen"
      statusBarTranslucent
      transparent
      visible={visible}
    >
      <View accessibilityViewIsModal style={styles.container}>
        <Pressable
          accessibilityLabel="Dismiss note actions"
          accessibilityRole="button"
          onPress={onClose}
          style={styles.scrim}
        />
        <View
          style={[
            styles.sheet,
            { paddingBottom: Math.max(insets.bottom, Spacing.md) },
          ]}
        >
          <Pressable
            accessibilityLabel="Close note actions"
            accessibilityRole="button"
            hitSlop={8}
            onPress={onClose}
            style={styles.handleTarget}
          >
            <View style={styles.handle} />
          </Pressable>
          <Pressable
            accessibilityRole="button"
            onPress={() => select(onExport)}
            style={({ pressed }) => [
              styles.action,
              pressed && styles.actionPressed,
            ]}
          >
            <Ionicons name="download-outline" size={20} color={Colors.ink} />
            <Text style={styles.actionLabel}>Export</Text>
          </Pressable>
          {!listening && !hasRecordingHistory && (
            <Pressable
              accessibilityRole="button"
              onPress={() => select(onImportRecording)}
              style={({ pressed }) => [
                styles.action,
                pressed && styles.actionPressed,
              ]}
            >
              <Ionicons
                name="cloud-upload-outline"
                size={20}
                color={Colors.ink}
              />
              <Text style={styles.actionLabel}>Import recording</Text>
            </Pressable>
          )}
          {(listening || !hasRecordingHistory) && (
            <Pressable
              accessibilityRole="button"
              onPress={() => select(onToggleListening)}
              style={({ pressed }) => [
                styles.action,
                pressed && styles.actionPressed,
              ]}
            >
              <Ionicons
                name={listening ? "mic-off-outline" : "mic-outline"}
                size={20}
                color={Colors.ink}
              />
              <Text style={styles.actionLabel}>{listeningLabel}</Text>
            </Pressable>
          )}
          <View style={styles.separator} />
          <Pressable
            accessibilityRole="button"
            onPress={() => select(onDelete)}
            style={({ pressed }) => [
              styles.action,
              pressed && styles.destructivePressed,
            ]}
          >
            <Ionicons
              name="trash-outline"
              size={20}
              color={Colors.destructive}
            />
            <Text style={styles.destructiveLabel}>Delete</Text>
          </Pressable>
        </View>
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    justifyContent: "flex-end",
  },
  scrim: {
    position: "absolute",
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    backgroundColor: Colors.scrim,
  },
  sheet: {
    borderWidth: StyleSheet.hairlineWidth,
    borderBottomWidth: 0,
    borderColor: Colors.border,
    borderTopLeftRadius: Radius.sheet,
    borderTopRightRadius: Radius.sheet,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.background,
    paddingHorizontal: Spacing.md,
    shadowColor: Colors.ink,
    shadowOffset: { width: 0, height: -4 },
    shadowOpacity: 0.12,
    shadowRadius: 16,
    elevation: 8,
  },
  handle: {
    width: 36,
    height: 4,
    borderRadius: 2,
    backgroundColor: Colors.border,
  },
  handleTarget: {
    height: 28,
    alignItems: "center",
    justifyContent: "center",
  },
  action: {
    minHeight: 52,
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
    borderRadius: Radius.control,
    borderCurve: CornerCurve.squircle,
    paddingHorizontal: Spacing.md,
  },
  actionPressed: {
    backgroundColor: Colors.accentSurface,
  },
  actionLabel: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  separator: {
    height: StyleSheet.hairlineWidth,
    marginVertical: Spacing.xs,
    backgroundColor: Colors.border,
  },
  destructivePressed: {
    backgroundColor: Colors.alert,
  },
  destructiveLabel: {
    ...Typography.bodyStrong,
    color: Colors.destructive,
  },
});
