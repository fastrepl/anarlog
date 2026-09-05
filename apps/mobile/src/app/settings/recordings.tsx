import { useRouter } from "expo-router";
import { FlatList, Pressable, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { IconButton } from "@/components/ui/icon-button";
import { Spacing, Typography } from "@/constants/theme";
import { useLiveQuery } from "@/db";
import { formatStorageBytes } from "@/settings/storage";
import { createStyleHook } from "@/settings/theme-provider";

export default function RecordingStorageScreen() {
  const styles = useStyles();
  const router = useRouter();
  const recordings = useLiveQuery<
    { id: string; title: string; size_bytes: number; availability: string },
    { id: string; title: string; size_bytes: number; availability: string }[]
  >({
    mapRows: (rows) => rows,
    sql: `
    SELECT session.id, session.title, attachment.size_bytes, COALESCE(local.availability, 'absent') AS availability
    FROM session_attachments AS attachment JOIN sessions AS session ON session.id = attachment.session_id
    LEFT JOIN attachment_local_state AS local ON local.attachment_id = attachment.id
    WHERE attachment.source_type = 'session_audio' AND attachment.deleted_at IS NULL AND session.deleted_at IS NULL
    ORDER BY session.created_at DESC, session.id DESC
  `,
  });
  return (
    <SafeAreaView style={styles.page}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="back"
          onPress={() => router.back()}
        />
        <Text style={styles.title}>Saved recordings</Text>
      </View>
      <FlatList
        data={recordings.data ?? []}
        keyExtractor={(item) => item.id}
        contentContainerStyle={styles.list}
        ListEmptyComponent={
          <Text style={styles.copy}>
            {recordings.error
              ? "Could not load recordings."
              : recordings.data
                ? "No saved recordings"
                : "Loading…"}
          </Text>
        }
        renderItem={({ item }) => (
          <Pressable
            accessibilityRole="button"
            style={styles.row}
            onPress={() =>
              router.push({ pathname: "/note/[id]", params: { id: item.id } })
            }
          >
            <Text style={styles.title}>{item.title.trim() || "Untitled"}</Text>
            <Text style={styles.copy}>
              {formatStorageBytes(item.size_bytes)} ·{" "}
              {item.availability === "present"
                ? "On this device"
                : "Not downloaded"}
            </Text>
          </Pressable>
        )}
      />
    </SafeAreaView>
  );
}

const useStyles = createStyleHook((Colors) => ({
  page: { flex: 1, backgroundColor: Colors.background },
  header: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
    padding: Spacing.lg,
  },
  title: { ...Typography.section, color: Colors.ink },
  copy: { ...Typography.caption, color: Colors.muted },
  list: { padding: Spacing.lg, gap: Spacing.md },
  row: {
    padding: Spacing.md,
    backgroundColor: Colors.surface,
    borderRadius: 16,
    gap: Spacing.sm,
  },
}));
