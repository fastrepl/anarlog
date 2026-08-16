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
  onPress,
  onDelete,
}: {
  session: TimelineSession;
  onPress: () => void;
  onDelete?: () => void;
}) {
  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [styles.card, pressed && styles.cardPressed]}
    >
      <View style={styles.row}>
        <Text
          style={[styles.title, !session.title && styles.titleEmpty]}
          numberOfLines={1}
        >
          {session.title || "Untitled"}
        </Text>
        {onDelete && (
          <Pressable
            accessibilityLabel={`Delete ${session.title || "Untitled"}`}
            accessibilityRole="button"
            hitSlop={4}
            onPress={(event) => {
              event.stopPropagation();
              onDelete();
            }}
            style={({ pressed }) => [
              styles.moreButton,
              pressed && styles.moreButtonPressed,
            ]}
          >
            <Ionicons
              name="ellipsis-horizontal"
              size={17}
              color={Colors.muted}
            />
          </Pressable>
        )}
      </View>
      <Text style={styles.subtitle}>{relativeLabel(session.startedAt)}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  card: {
    minHeight: 64,
    justifyContent: "center",
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    paddingHorizontal: Spacing.compact,
    paddingVertical: Spacing.sm,
    marginBottom: Spacing.sm,
    backgroundColor: Colors.surface,
  },
  cardPressed: {
    backgroundColor: Colors.accentSurface,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: {
    flex: 1,
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  titleEmpty: {
    color: Colors.muted,
  },
  moreButton: {
    width: 32,
    height: 32,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.pill,
    marginLeft: Spacing.sm,
  },
  moreButtonPressed: {
    backgroundColor: Colors.accentSurface,
  },
  subtitle: {
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
});
