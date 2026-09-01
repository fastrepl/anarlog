import MenuView, { type MenuAction } from "@expo/ui/community/menu";
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

const DELETE_ACTIONS: MenuAction[] = [
  {
    id: "delete",
    title: "Delete",
    image: "trash",
    attributes: { destructive: true },
  },
];

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

  const content = (
    <Pressable
      accessibilityHint={onDelete ? "Long press for actions" : undefined}
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
  );

  if (!onDelete) return <View style={styles.card}>{content}</View>;

  return (
    <MenuView
      actions={DELETE_ACTIONS}
      onPressAction={({ nativeEvent }) => {
        if (nativeEvent.event === "delete") onDelete();
      }}
      shouldOpenOnLongPress
      style={styles.card}
    >
      {content}
    </MenuView>
  );
}

const styles = StyleSheet.create({
  card: {
    minHeight: 64,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    marginBottom: Spacing.sm,
    backgroundColor: Colors.surface,
    overflow: "hidden",
  },
  cardContent: {
    minHeight: 64,
    justifyContent: "center",
    paddingHorizontal: Spacing.compact,
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
