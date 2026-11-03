import { BottomSheet, Host, RNHostView } from "@expo/ui";
import { useMutation } from "@tanstack/react-query";
import {
  ActivityIndicator,
  FlatList,
  Text as NativeText,
  View,
} from "react-native";

import { Button } from "@/components/ui/button";
import { IconButton } from "@/components/ui/icon-button";
import { Spacing, Typography } from "@/constants/theme";
import {
  restoreHistoryEntry,
  useSessionVersionHistory,
  type RestoredNote,
  type VersionHistoryEntry,
} from "@/data/conflicts";
import { relativeLabel } from "@/data/timeline-model";
import { Text } from "@/i18n";
import {
  createStyleHook,
  useAppColorScheme,
  useColors,
} from "@/settings/theme-provider";

export function VersionHistorySheet({
  sessionId,
  visible,
  onBeforeRestore,
  onClose,
  onRestored,
}: {
  sessionId: string;
  visible: boolean;
  onBeforeRestore: () => void | Promise<void>;
  onClose: () => void;
  onRestored: (restored: RestoredNote) => void;
}) {
  const colorScheme = useAppColorScheme();
  return (
    <Host
      colorScheme={colorScheme}
      style={{ position: "absolute" }}
      pointerEvents="none"
    >
      <BottomSheet
        isPresented={visible}
        onDismiss={onClose}
        snapPoints={["half", "full"]}
      >
        <RNHostView>
          {visible ? (
            <VersionHistory
              key={sessionId}
              sessionId={sessionId}
              onBeforeRestore={onBeforeRestore}
              onClose={onClose}
              onRestored={onRestored}
            />
          ) : (
            <View />
          )}
        </RNHostView>
      </BottomSheet>
    </Host>
  );
}

function VersionHistory({
  sessionId,
  onBeforeRestore,
  onClose,
  onRestored,
}: {
  sessionId: string;
  onBeforeRestore: () => void | Promise<void>;
  onClose: () => void;
  onRestored: (restored: RestoredNote) => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const history = useSessionVersionHistory(sessionId);
  const restore = useMutation({
    mutationFn: async (entry: VersionHistoryEntry) => {
      await onBeforeRestore();
      return restoreHistoryEntry(sessionId, entry);
    },
    onSuccess: (restored) => {
      onRestored(restored);
      onClose();
    },
  });

  return (
    <View style={styles.content}>
      <View style={styles.header}>
        <Text accessibilityRole="header" style={styles.title}>
          Version history
        </Text>
        <IconButton
          accessibilityLabel="Close version history"
          icon="close"
          onPress={onClose}
        />
      </View>
      {history.isLoading && <ActivityIndicator color={Colors.muted} />}
      {(history.error || restore.error) && (
        <Text accessibilityRole="alert" style={styles.error}>
          {restore.error
            ? "Couldn’t restore this version. Try again."
            : "Couldn’t load earlier versions. Close and try again."}
        </Text>
      )}
      <FlatList
        data={history.data}
        keyExtractor={(entry) => entry.id}
        contentContainerStyle={styles.list}
        ListEmptyComponent={
          history.isLoading ? null : (
            <Text style={styles.empty}>
              Earlier versions of this note appear here as you edit it.
            </Text>
          )
        }
        renderItem={({ item }) => (
          <View style={styles.row}>
            <View style={styles.rowCopy}>
              <Text style={styles.rowTitle}>
                {item.label} · {relativeLabel(item.at)}
              </Text>
              <Text numberOfLines={2} style={styles.rowPreview}>
                {item.preview || "Empty note"}
              </Text>
            </View>
            <Button
              label="Restore"
              disabled={restore.isPending}
              onPress={() => restore.mutate(item)}
              size="small"
              variant="outline"
            />
          </View>
        )}
      />
    </View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  content: { flex: 1, gap: Spacing.sm, paddingBottom: Spacing.md },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: { ...Typography.section, color: Colors.ink },
  list: { gap: Spacing.md, paddingBottom: Spacing.md },
  row: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
  },
  rowCopy: { flex: 1 },
  rowTitle: { ...Typography.captionStrong, color: Colors.ink },
  rowPreview: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  empty: {
    ...Typography.body,
    color: Colors.muted,
    paddingVertical: Spacing.md,
  },
  error: { ...Typography.caption, color: Colors.destructive },
}));
