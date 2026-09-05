import {
  GlassView,
  isGlassEffectAPIAvailable,
  isLiquidGlassAvailable,
} from "expo-glass-effect";
import { useRef, useState } from "react";
import {
  FlatList,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";

import { SessionCard } from "@/components/session-card";
import { IconButton } from "@/components/ui/icon-button";
import { NativeIcon } from "@/components/ui/native-icon";
import { CornerCurve, Radius, Spacing, Typography } from "@/constants/theme";
import { useSessionSearch } from "@/data/search";
import { useSidebarItemPreferences } from "@/data/sidebar-preferences";
import type { TimelineSession } from "@/data/timeline";
import { captureAnalytics } from "@/lib/analytics";
import { useMountEffect } from "@/lib/use-mount-effect";
import {
  createStyleHook,
  useAppColorScheme,
  useColors,
} from "@/settings/theme-provider";

function SearchAnalytics({ resultCount }: { resultCount: number }) {
  useMountEffect(() => {
    captureAnalytics("search_performed", {
      entry_point: "mobile_home",
      result_count: resultCount,
      entity_types: ["note"],
    });
  });
  return null;
}

export function SearchPalette({
  onClose,
  onOpenSession,
  onDeleteSession,
}: {
  onClose: () => void;
  onOpenSession: (session: TimelineSession) => void;
  onDeleteSession: (session: TimelineSession) => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const colorScheme = useAppColorScheme();
  const insets = useSafeAreaInsets();
  const sidebarPreferences = useSidebarItemPreferences();
  const [query, setQuery] = useState("");
  const [settledQuery, setSettledQuery] = useState("");
  const search = useSessionSearch(query);
  const inputRef = useRef<TextInput>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hasQuery = query.trim() !== "";
  const hasGlass = isGlassEffectAPIAvailable() && isLiquidGlassAvailable();

  useMountEffect(() => () => {
    if (timerRef.current) clearTimeout(timerRef.current);
  });

  const handleSearchChange = (value: string) => {
    setQuery(value);
    if (timerRef.current) clearTimeout(timerRef.current);
    const term = value.trim();
    if (!term) {
      timerRef.current = null;
      setSettledQuery("");
      return;
    }
    timerRef.current = setTimeout(() => {
      timerRef.current = null;
      setSettledQuery(term);
    }, 300);
  };

  return (
    <Modal
      transparent
      visible
      onRequestClose={onClose}
      onShow={() => inputRef.current?.focus()}
      statusBarTranslucent
      navigationBarTranslucent
    >
      <Pressable accessible={false} onPress={onClose} style={styles.backdrop} />
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        pointerEvents="box-none"
        style={styles.keyboard}
      >
        <View
          pointerEvents="box-none"
          style={[
            styles.overlay,
            {
              paddingTop: insets.top + Spacing.md,
              paddingBottom: insets.bottom + Spacing.md,
            },
          ]}
        >
          <View
            accessibilityViewIsModal
            onAccessibilityEscape={onClose}
            style={[styles.palette, !hasGlass && styles.fallback]}
          >
            {hasGlass && (
              <GlassView
                colorScheme={colorScheme}
                glassEffectStyle="regular"
                pointerEvents="none"
                style={StyleSheet.absoluteFill}
              />
            )}
            <View style={styles.searchBar}>
              <NativeIcon name="search" size={16} color={Colors.muted} />
              <TextInput
                ref={inputRef}
                accessibilityLabel="Search meetings"
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={handleSearchChange}
                placeholder="Search meetings"
                placeholderTextColor={Colors.muted}
                returnKeyType="search"
                style={styles.input}
                value={query}
              />
              {query !== "" && (
                <IconButton
                  accessibilityLabel="Clear search"
                  icon="close"
                  iconSize={16}
                  onPress={() => {
                    handleSearchChange("");
                    inputRef.current?.focus();
                  }}
                  tone="muted"
                />
              )}
            </View>
            {settledQuery !== "" &&
              settledQuery === query.trim() &&
              !search.isLoading && (
                <SearchAnalytics
                  key={settledQuery}
                  resultCount={search.results.length}
                />
              )}
            <FlatList
              data={search.results}
              keyExtractor={(session) => session.id}
              keyboardShouldPersistTaps="handled"
              keyboardDismissMode="on-drag"
              style={styles.results}
              contentContainerStyle={styles.resultContent}
              ListEmptyComponent={
                <Text style={styles.empty} accessibilityLiveRegion="polite">
                  {!hasQuery
                    ? "Search by title or note content"
                    : search.isLoading
                      ? "Searching…"
                      : "No matches"}
                </Text>
              }
              renderItem={({ item }) => (
                <SessionCard
                  session={item}
                  showFolder={sidebarPreferences.showFolder}
                  showTags={sidebarPreferences.showTags}
                  variant="plain"
                  onPress={() => {
                    captureAnalytics("search_result_opened", {
                      entry_point: "mobile_home",
                      result_type: "session",
                    });
                    onOpenSession(item);
                  }}
                  onDelete={() => onDeleteSession(item)}
                />
              )}
            />
          </View>
        </View>
      </KeyboardAvoidingView>
    </Modal>
  );
}

const useStyles = createStyleHook((Colors) => ({
  backdrop: {
    ...StyleSheet.absoluteFill,
    backgroundColor: Colors.scrim,
  },
  overlay: {
    flex: 1,
    paddingHorizontal: Spacing.md,
  },
  keyboard: {
    flex: 1,
  },
  palette: {
    width: "100%",
    maxWidth: 560,
    maxHeight: "100%",
    alignSelf: "center",
    flexShrink: 1,
    borderRadius: Radius.panel,
    borderCurve: CornerCurve.squircle,
    overflow: "hidden",
  },
  fallback: {
    backgroundColor: Colors.surface,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
  },
  searchBar: {
    flexDirection: "row",
    alignItems: "center",
    paddingLeft: Spacing.md,
    paddingRight: Spacing.sm,
    paddingVertical: Spacing.sm,
    gap: Spacing.xs,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: Colors.border,
  },
  input: {
    flex: 1,
    minHeight: 44,
    paddingHorizontal: Spacing.sm,
    transform: [{ translateY: -2 }],
    ...Typography.body,
    color: Colors.ink,
  },
  results: {
    flexGrow: 0,
    flexShrink: 1,
    maxHeight: 440,
  },
  resultContent: {
    padding: Spacing.sm,
  },
  empty: {
    padding: Spacing.md,
    ...Typography.body,
    color: Colors.muted,
    textAlign: "center",
  },
}));
