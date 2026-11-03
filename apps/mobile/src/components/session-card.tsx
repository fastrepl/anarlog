import MenuView from "@expo/ui/community/menu";
import { Ionicons } from "@expo/vector-icons";
import { useRef, useState } from "react";
import { Pressable, StyleSheet, Text as NativeText, View } from "react-native";
import ReanimatedSwipeable, {
  type SwipeableMethods,
} from "react-native-gesture-handler/ReanimatedSwipeable";

import { CornerCurve, Radius, Spacing, Typography } from "@/constants/theme";
import { relativeLabel, type TimelineSession } from "@/data/timeline";
import { Text, translate, useLanguage } from "@/i18n";
import { createStyleHook, useColors } from "@/settings/theme-provider";

const SWIPE_ACTION_WIDTH = 88;

export function SessionCard({
  session,
  showFolder,
  showTags,
  onPress,
  onDelete,
  variant = "card",
}: {
  session: TimelineSession;
  showFolder: boolean;
  showTags: boolean;
  onPress: () => void;
  onDelete?: () => void;
  variant?: "card" | "plain";
}) {
  const styles = useStyles();
  const Colors = useColors();
  const { language } = useLanguage();
  const [width, setWidth] = useState<number>();
  const swipeableRef = useRef<SwipeableMethods>(null);
  const title = session.title || "Untitled";
  const folder = showFolder ? session.folderPath : "";
  const tags = showTags ? session.tags.map((tag) => `#${tag}`).join(" ") : "";
  const accessibilityLabel = [
    title,
    folder ? translate(`Folder ${folder}`, language) : "",
    relativeLabel(session.startedAt),
    tags,
  ]
    .filter(Boolean)
    .join(", ");

  const content = (
    <Pressable
      accessibilityHint={
        onDelete
          ? translate("Long press or swipe left for actions", language)
          : undefined
      }
      accessibilityLabel={translate(accessibilityLabel, language)}
      accessibilityRole="button"
      onPress={onPress}
      style={({ pressed }) => [
        styles.card,
        styles.cardContent,
        variant === "plain" && styles.cardPlain,
        { width },
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

  const row = (
    <View
      onLayout={({ nativeEvent }) => {
        const nextWidth = nativeEvent.layout.width;
        // Native MenuView measures its trigger on mount, so wait for this width.
        setWidth((current) => (current === nextWidth ? current : nextWidth));
      }}
      style={styles.row}
    >
      {onDelete && width != null ? (
        <MenuView
          key={width}
          actions={[
            {
              id: "delete",
              title: translate("Delete", language),
              image: "trash",
              attributes: { destructive: true },
            },
          ]}
          onPressAction={({ nativeEvent }) => {
            if (nativeEvent.event === "delete") onDelete();
          }}
          shouldOpenOnLongPress
        >
          {content}
        </MenuView>
      ) : (
        content
      )}
    </View>
  );

  if (!onDelete) return row;

  return (
    <ReanimatedSwipeable
      ref={swipeableRef}
      friction={2}
      overshootRight={false}
      rightThreshold={SWIPE_ACTION_WIDTH / 2}
      renderRightActions={() => (
        <Pressable
          accessibilityLabel={translate(`Delete ${title}`, language)}
          accessibilityRole="button"
          onPress={() => {
            swipeableRef.current?.close();
            onDelete();
          }}
          style={({ pressed }) => [
            styles.swipeAction,
            pressed && styles.swipeActionPressed,
          ]}
        >
          <Ionicons
            name="trash-outline"
            size={20}
            color={Colors.destructiveForeground}
          />
          <Text style={styles.swipeActionLabel}>Delete</Text>
        </Pressable>
      )}
    >
      {row}
    </ReanimatedSwipeable>
  );
}

const useStyles = createStyleHook((Colors) => ({
  row: {
    alignSelf: "stretch",
    marginBottom: Spacing.sm,
  },
  card: {
    minHeight: 64,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
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
  swipeAction: {
    width: SWIPE_ACTION_WIDTH,
    marginLeft: Spacing.sm,
    marginBottom: Spacing.sm,
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.xs,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    backgroundColor: Colors.destructive,
  },
  swipeActionPressed: {
    opacity: 0.8,
  },
  swipeActionLabel: {
    ...Typography.caption,
    color: Colors.destructiveForeground,
  },
  cardPlain: {
    borderColor: "transparent",
    backgroundColor: "transparent",
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
}));
