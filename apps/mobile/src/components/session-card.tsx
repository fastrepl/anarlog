import { Pressable, StyleSheet, Text, View } from "react-native";

import { Colors, CornerCurve, Radius, Spacing } from "@/constants/theme";
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
          <Pressable hitSlop={8} onPress={onDelete}>
            <Text style={styles.ellipsis}>…</Text>
          </Pressable>
        )}
      </View>
      <Text style={styles.subtitle}>{relativeLabel(session.startedAt)}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  card: {
    borderWidth: 1.5,
    borderColor: Colors.ink,
    borderRadius: Radius.card,
    borderCurve: CornerCurve.squircle,
    paddingHorizontal: Spacing.md,
    paddingVertical: Spacing.md,
    marginBottom: Spacing.md,
    backgroundColor: Colors.paper,
  },
  cardPressed: {
    opacity: 0.6,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: {
    flex: 1,
    fontSize: 17,
    fontWeight: "600",
    color: Colors.ink,
  },
  titleEmpty: {
    color: Colors.muted,
  },
  ellipsis: {
    fontSize: 17,
    color: Colors.muted,
    marginLeft: Spacing.sm,
  },
  subtitle: {
    marginTop: Spacing.xs,
    fontSize: 13,
    color: Colors.muted,
  },
});
