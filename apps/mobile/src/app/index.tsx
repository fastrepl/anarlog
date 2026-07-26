import { Ionicons } from "@expo/vector-icons";
import { useRouter } from "expo-router";
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { SessionCard } from "@/components/session-card";
import { Colors, Radius, Spacing } from "@/constants/theme";
import { buildSessionList, mockSessions } from "@/data/sessions";

export default function HomeScreen() {
  const router = useRouter();
  const items = buildSessionList(mockSessions);

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <View style={styles.avatar} />
        <Pressable hitSlop={8}>
          <Ionicons name="search" size={22} color={Colors.ink} />
        </Pressable>
      </View>

      <ScrollView
        style={styles.list}
        contentContainerStyle={styles.listContent}
      >
        {items.map((item) => {
          if (item.type === "header") {
            return (
              <Text key={item.key} style={styles.sectionLabel}>
                {item.label}
              </Text>
            );
          }
          if (item.type === "now") {
            return (
              <View key={item.key} style={styles.nowDivider}>
                <View style={styles.nowDot} />
                <View style={styles.nowLine} />
              </View>
            );
          }
          return (
            <SessionCard
              key={item.key}
              session={item.session}
              onPress={() => router.push(`/note/${item.session.id}`)}
            />
          );
        })}
      </ScrollView>

      <Pressable
        onPress={() => router.push("/note/new")}
        style={({ pressed }) => [
          styles.listenButton,
          pressed && styles.listenButtonPressed,
        ]}
      >
        <View style={styles.listenDot} />
        <Text style={styles.listenLabel}>Start listening</Text>
      </Pressable>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.paper,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.lg,
    paddingVertical: Spacing.md,
  },
  avatar: {
    width: 32,
    height: 32,
    borderRadius: 16,
    borderWidth: 1.5,
    borderColor: Colors.ink,
  },
  list: {
    flex: 1,
  },
  listContent: {
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.lg,
  },
  sectionLabel: {
    fontSize: 15,
    fontWeight: "600",
    color: Colors.ink,
    marginTop: Spacing.md,
    marginBottom: Spacing.sm,
  },
  nowDivider: {
    flexDirection: "row",
    alignItems: "center",
    marginBottom: Spacing.md,
    marginLeft: -Spacing.lg,
  },
  nowDot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    backgroundColor: Colors.accent,
  },
  nowLine: {
    flex: 1,
    height: 3,
    backgroundColor: Colors.accent,
  },
  listenButton: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginBottom: Spacing.md,
    paddingVertical: Spacing.lg,
    borderRadius: Radius.pill,
    backgroundColor: Colors.ink,
  },
  listenButtonPressed: {
    opacity: 0.85,
  },
  listenDot: {
    width: 12,
    height: 12,
    borderRadius: 6,
    backgroundColor: Colors.accent,
  },
  listenLabel: {
    fontSize: 18,
    fontWeight: "600",
    color: Colors.inkInverse,
  },
});
