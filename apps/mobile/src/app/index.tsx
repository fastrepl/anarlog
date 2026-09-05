import AsyncStorage from "@react-native-async-storage/async-storage";
import { useRouter } from "expo-router";
import { useRef, useState } from "react";
import { Platform, ScrollView, StyleSheet, Text, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useAuth } from "@/auth/context";
import { ActionButtonCard } from "@/components/action-button-card";
import { SearchPalette } from "@/components/search-palette";
import { SessionCard } from "@/components/session-card";
import { StartListeningButton } from "@/components/start-listening-button";
import { IconButton } from "@/components/ui/icon-button";
import { UserAvatarButton } from "@/components/user-avatar";
import { Colors, Spacing, Typography } from "@/constants/theme";
import { createSession, deleteSession } from "@/data/session";
import { useSidebarItemPreferences } from "@/data/sidebar-preferences";
import { useTimelineSessions, type TimelineSession } from "@/data/timeline";
import { confirmDestructive } from "@/lib/confirm";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

const ACTION_BUTTON_CARD_DISMISSED_KEY = "action-button-card-dismissed";

export default function HomeScreen() {
  const router = useRouter();
  const auth = useAuth();
  const { items, isLoading } = useTimelineSessions();
  const sidebarPreferences = useSidebarItemPreferences();
  const [searching, setSearching] = useState(false);
  const [showActionButtonCard, setShowActionButtonCard] = useState(false);
  // Ref, not state: two taps in the same frame both pass a state check.
  const busyRef = useRef(false);

  useMountEffect(() => {
    if (Platform.OS !== "ios") return;
    let active = true;
    void AsyncStorage.getItem(ACTION_BUTTON_CARD_DISMISSED_KEY).then(
      (dismissed) => {
        if (active && dismissed !== "1") setShowActionButtonCard(true);
      },
      (error) => {
        if (active) setShowActionButtonCard(true);
        captureOperationalError(error, {
          operation: "action_button_card_load",
          level: "warning",
        });
      },
    );
    return () => {
      active = false;
    };
  });

  const handleDelete = async (session: TimelineSession) => {
    const confirmed = await confirmDestructive(
      `Delete "${session.title || "Untitled"}"?`,
      "Delete",
    );
    if (!confirmed) return;
    try {
      await deleteSession(session.id);
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_delete",
        tags: { entry_point: "mobile_home" },
      });
    }
  };

  const createAndOpen = async (query = "") => {
    if (busyRef.current) return;
    busyRef.current = true;
    try {
      const sessionId = await createSession({
        entryPoint: query.includes("listen=1") ? "start_listening" : "new_note",
        ownerUserId: auth.session?.user.id,
      });
      router.push(`/note/${sessionId}${query}`);
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_create",
        tags: {
          entry_point: query.includes("listen=1")
            ? "start_listening"
            : "new_note",
        },
      });
    } finally {
      busyRef.current = false;
    }
  };

  const dismissActionButtonCard = () => {
    setShowActionButtonCard(false);
    void AsyncStorage.setItem(ACTION_BUTTON_CARD_DISMISSED_KEY, "1").catch(
      (error) => {
        captureOperationalError(error, {
          operation: "action_button_card_dismiss",
          level: "warning",
        });
      },
    );
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <UserAvatarButton
          accessibilityLabel="Open settings"
          onPress={() => router.push("/settings")}
          user={auth.session?.user ?? null}
        />
        <View style={styles.headerActions}>
          <IconButton
            accessibilityLabel="Create new note"
            icon="new-note"
            onPress={() => void createAndOpen()}
          />
          <IconButton
            accessibilityLabel="Search meetings"
            icon="search"
            onPress={() => setSearching(true)}
          />
        </View>
      </View>

      <ScrollView
        style={styles.list}
        contentContainerStyle={styles.listContent}
        keyboardShouldPersistTaps="handled"
      >
        {showActionButtonCard && (
          <ActionButtonCard
            onConfigure={() => router.push("/action-button")}
            onDismiss={dismissActionButtonCard}
          />
        )}
        {!isLoading && items.length === 0 && (
          <View style={styles.empty}>
            <Text style={styles.emptyTitle}>No meetings yet</Text>
            <Text style={styles.emptyBody}>
              Start listening or create a new note.
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
          return (
            <SessionCard
              key={item.key}
              session={item.session}
              showFolder={sidebarPreferences.showFolder}
              showTags={sidebarPreferences.showTags}
              onPress={() => router.push(`/note/${item.session.id}`)}
              onDelete={() => void handleDelete(item.session)}
            />
          );
        })}
      </ScrollView>

      <StartListeningButton
        bottomSpacing={Spacing.xs}
        onPress={() => void createAndOpen("?listen=1")}
      />
      {searching && (
        <SearchPalette
          onClose={() => setSearching(false)}
          onOpenSession={(session) => {
            setSearching(false);
            router.push(`/note/${session.id}`);
          }}
          onDeleteSession={(session) => {
            setSearching(false);
            requestAnimationFrame(() => {
              void handleDelete(session);
            });
          }}
        />
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.sm,
    paddingBottom: Spacing.md,
  },
  headerActions: {
    flexDirection: "row",
    gap: Spacing.xs,
  },
  list: {
    flex: 1,
  },
  listContent: {
    flexGrow: 1,
    paddingHorizontal: Spacing.lg,
    paddingBottom: Spacing.lg,
  },
  empty: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    paddingHorizontal: Spacing.lg,
    gap: Spacing.sm,
  },
  emptyTitle: {
    ...Typography.bodyStrong,
    color: Colors.ink,
  },
  emptyBody: {
    ...Typography.body,
    color: Colors.muted,
    textAlign: "center",
  },
  sectionLabel: {
    ...Typography.section,
    color: Colors.muted,
    marginTop: Spacing.md,
    marginBottom: Spacing.sm,
  },
});
