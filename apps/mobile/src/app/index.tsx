import { Ionicons } from "@expo/vector-icons";
import { useRouter } from "expo-router";
import { useEffect, useRef, useState } from "react";
import {
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { ProfileSheet } from "@/components/profile-sheet";
import { SessionCard } from "@/components/session-card";
import { Colors, Radius, Spacing } from "@/constants/theme";
import { importVoiceMemos } from "@/data/import-voice-memo";
import { useSessionSearch } from "@/data/search";
import { createSession, deleteSession } from "@/data/session";
import { useTimelineSessions, type TimelineSession } from "@/data/timeline";
import { captureAnalytics } from "@/lib/analytics";
import { confirmDestructive } from "@/lib/confirm";

export default function HomeScreen() {
  const router = useRouter();
  const { items, isLoading } = useTimelineSessions();
  const [profileOpen, setProfileOpen] = useState(false);
  const [importing, setImporting] = useState(false);
  const [query, setQuery] = useState<string | null>(null);
  const searching = query !== null;
  const search = useSessionSearch(query ?? "");
  // Ref, not state: two taps in the same frame both pass a state check.
  const busyRef = useRef(false);

  useEffect(() => {
    if (!searching || !query?.trim() || search.isLoading) return;
    const timeout = setTimeout(() => {
      captureAnalytics("search_performed", {
        entry_point: "mobile_home",
        result_count: search.results.length,
        entity_types: ["note"],
      });
    }, 300);
    return () => clearTimeout(timeout);
  }, [query, search.isLoading, search.results.length, searching]);

  const handleDelete = async (session: TimelineSession) => {
    const confirmed = await confirmDestructive(
      `Delete "${session.title || "Untitled"}"?`,
      "Delete",
    );
    if (confirmed) void deleteSession(session.id);
  };

  const createAndOpen = async (query = "") => {
    if (busyRef.current) return;
    busyRef.current = true;
    try {
      const sessionId = await createSession({
        entryPoint: query.includes("listen=1") ? "start_listening" : "new_note",
      });
      router.push(`/note/${sessionId}${query}`);
    } finally {
      busyRef.current = false;
    }
  };

  const handleImport = async () => {
    if (busyRef.current) return;
    busyRef.current = true;
    setImporting(true);
    try {
      const sessionIds = await importVoiceMemos();
      if (sessionIds.length === 1) {
        router.push(`/note/${sessionIds[0]}`);
      }
    } finally {
      busyRef.current = false;
      setImporting(false);
    }
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        {searching ? (
          <>
            <TextInput
              style={styles.searchInput}
              autoFocus
              value={query ?? ""}
              onChangeText={setQuery}
              placeholder="Search notes"
              placeholderTextColor={Colors.muted}
            />
            <Pressable hitSlop={8} onPress={() => setQuery(null)}>
              <Ionicons name="close" size={22} color={Colors.ink} />
            </Pressable>
          </>
        ) : (
          <>
            <Pressable hitSlop={8} onPress={() => setProfileOpen(true)}>
              <View style={styles.avatar} />
            </Pressable>
            <Pressable hitSlop={8} onPress={() => setQuery("")}>
              <Ionicons name="search" size={22} color={Colors.ink} />
            </Pressable>
          </>
        )}
      </View>

      <ScrollView
        style={styles.list}
        contentContainerStyle={styles.listContent}
        keyboardShouldPersistTaps="handled"
      >
        {searching ? (
          <>
            {query.trim() !== "" &&
              !search.isLoading &&
              search.results.length === 0 && (
                <View style={styles.empty}>
                  <Text style={styles.emptyBody}>No matches</Text>
                </View>
              )}
            {search.results.map((session) => (
              <SessionCard
                key={session.id}
                session={session}
                onPress={() => {
                  captureAnalytics("search_result_opened", {
                    entry_point: "mobile_home",
                    result_type: "session",
                  });
                  router.push(`/note/${session.id}`);
                }}
                onDelete={() => void handleDelete(session)}
              />
            ))}
          </>
        ) : (
          <>
            {!isLoading && items.length === 0 && (
              <View style={styles.empty}>
                <Text style={styles.emptyTitle}>No notes yet</Text>
                <Text style={styles.emptyBody}>
                  Start listening, write a note, or import a voice memo.
                </Text>
              </View>
            )}
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
                  onDelete={() => void handleDelete(item.session)}
                />
              );
            })}
          </>
        )}
      </ScrollView>

      <View style={styles.actions}>
        <Pressable
          onPress={() => void createAndOpen()}
          style={({ pressed }) => [
            styles.secondaryButton,
            pressed && styles.pressed,
          ]}
        >
          <Ionicons name="create-outline" size={18} color={Colors.ink} />
          <Text style={styles.secondaryLabel}>New note</Text>
        </Pressable>
        <Pressable
          onPress={handleImport}
          disabled={importing}
          style={({ pressed }) => [
            styles.secondaryButton,
            (pressed || importing) && styles.pressed,
          ]}
        >
          <Ionicons name="cloud-upload-outline" size={18} color={Colors.ink} />
          <Text style={styles.secondaryLabel}>
            {importing ? "Importing…" : "Import memo"}
          </Text>
        </Pressable>
      </View>

      <Pressable
        onPress={() => void createAndOpen("?listen=1")}
        style={({ pressed }) => [
          styles.listenButton,
          pressed && styles.pressed,
        ]}
      >
        <View style={styles.listenDot} />
        <Text style={styles.listenLabel}>Start listening</Text>
      </Pressable>

      <ProfileSheet
        visible={profileOpen}
        onClose={() => setProfileOpen(false)}
      />
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
  searchInput: {
    flex: 1,
    marginRight: Spacing.sm,
    fontSize: 17,
    color: Colors.ink,
    paddingVertical: 0,
  },
  list: {
    flex: 1,
  },
  listContent: {
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.lg,
  },
  empty: {
    alignItems: "center",
    paddingVertical: Spacing.xl * 2,
    gap: Spacing.sm,
  },
  emptyTitle: {
    fontSize: 17,
    fontWeight: "600",
    color: Colors.ink,
  },
  emptyBody: {
    fontSize: 14,
    color: Colors.muted,
    textAlign: "center",
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
  actions: {
    flexDirection: "row",
    gap: Spacing.sm,
    marginHorizontal: Spacing.lg,
    marginBottom: Spacing.sm,
  },
  secondaryButton: {
    flex: 1,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: Spacing.xs,
    borderWidth: 1.5,
    borderColor: Colors.ink,
    borderRadius: Radius.pill,
    paddingVertical: Spacing.sm + 2,
    backgroundColor: Colors.paper,
  },
  secondaryLabel: {
    fontSize: 14,
    fontWeight: "600",
    color: Colors.ink,
  },
  pressed: {
    opacity: 0.6,
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
