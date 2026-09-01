import { Ionicons } from "@expo/vector-icons";
import { Pressable, StyleSheet, Text, View } from "react-native";

import {
  Colors,
  CornerCurve,
  Radius,
  Spacing,
  Typography,
} from "@/constants/theme";
import { relativeLabel, type TimelineSession } from "@/data/timeline";

export function SessionCard({
  session,
  showFolder,
  showTags,
  onPress,
  onDelete,
}: {
  session: TimelineSession;
  showFolder: boolean;
  showTags: boolean;
  onPress: () => void;
  onDelete?: () => void;
}) {
  const title = session.title || "Untitled";
  const folder = showFolder ? session.folderPath : "";
  const tags = showTags ? session.tags.map((tag) => `#${tag}`).join(" ") : "";
  const accessibilityLabel = [
    title,
    folder ? `Folder ${folder}` : "",
    relativeLabel(session.startedAt),
    tags,
  ]
    .filter(Boolean)
    .join(", ");

  return (
    <View style={styles.card}>
      <Pressable
        accessibilityLabel={accessibilityLabel}
        accessibilityRole="button"
        onPress={onPress}
        style={({ pressed }) => [
          styles.cardContent,
          pressed && styles.cardPressed,
        ]}
      >
        {folder && (
          <View style={styles.folderRow}>
            <Ionicons name="folder-outline" size={12} color={Colors.muted} />
            <Text style={styles.folder} numberOfLines={1}>
              {folder}
            </Text>
          </View>
        )}
        <Text
          style={[styles.title, !session.title && styles.titleEmpty]}
          numberOfLines={1}
        >
          {title}
        </Text>
        <Text style={styles.subtitle}>{relativeLabel(session.startedAt)}</Text>
        {tags && (
          <Text style={styles.tags} numberOfLines={1}>
            {tags}
          </Text>
        )}
      </Pressable>
      {onDelete && (
        <Pressable
          accessibilityLabel={`Delete ${title}`}
          accessibilityRole="button"
          hitSlop={4}
          onPress={onDelete}
          style={({ pressed }) => [
            styles.moreButton,
            pressed && styles.moreButtonPressed,
          ]}
        >
          <Ionicons name="ellipsis-horizontal" size={17} color={Colors.muted} />
        </Pressable>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    minHeight: 64,
    flexDirection: "row",
    alignItems: "stretch",
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    marginBottom: Spacing.sm,
    backgroundColor: Colors.surface,
    overflow: "hidden",
  },
  cardContent: {
    flex: 1,
    justifyContent: "center",
    paddingLeft: Spacing.compact,
    paddingVertical: Spacing.sm,
  },
  cardPressed: {
    backgroundColor: Colors.accentSurface,
  },
  title: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  folderRow: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginBottom: Spacing.xs,
  },
  folder: {
    flex: 1,
    ...Typography.caption,
    color: Colors.muted,
  },
  titleEmpty: {
    color: Colors.muted,
  },
  moreButton: {
    width: 32,
    alignSelf: "stretch",
    alignItems: "center",
    justifyContent: "center",
    marginHorizontal: Spacing.compact,
    marginVertical: Spacing.sm,
    borderRadius: Radius.pill,
  },
  moreButtonPressed: {
    backgroundColor: Colors.accentSurface,
  },
  subtitle: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  tags: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
});
