import { BottomSheet, Host, RNHostView } from "@expo/ui";
import { Ionicons } from "@expo/vector-icons";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import {
  ActivityIndicator,
  FlatList,
  Keyboard,
  Pressable,
  Text as NativeText,
  TextInput,
  View,
} from "react-native";

import { normalizeFolderPath } from "@anlg/utils/folders";

import { IconButton } from "@/components/ui/icon-button";
import { Radius, Spacing, Typography } from "@/constants/theme";
import {
  saveSessionFolder,
  useFolderPaths,
  useSessionFolder,
} from "@/data/folders";
import { Text, translate, useLanguage } from "@/i18n";
import {
  createStyleHook,
  useColors,
  useAppColorScheme,
} from "@/settings/theme-provider";

export function FolderPickerSheet({
  sessionId,
  visible,
  onClose,
}: {
  sessionId: string;
  visible: boolean;
  onClose: () => void;
}) {
  const colorScheme = useAppColorScheme();
  return (
    <Host
      colorScheme={colorScheme}
      style={{ position: "absolute" }}
      pointerEvents="none"
    >
      <BottomSheet
        isPresented={visible}
        onDismiss={onClose}
        snapPoints={["half", "full"]}
      >
        <RNHostView>
          {visible ? (
            <FolderPicker
              key={sessionId}
              sessionId={sessionId}
              onClose={onClose}
            />
          ) : (
            <View />
          )}
        </RNHostView>
      </BottomSheet>
    </Host>
  );
}

function FolderPicker({
  sessionId,
  onClose,
}: {
  sessionId: string;
  onClose: () => void;
}) {
  const { language } = useLanguage();
  const styles = useStyles();
  const Colors = useColors();
  const [query, setQuery] = useState("");
  const folders = useFolderPaths();
  const assigned = useSessionFolder(sessionId);
  const current = assigned.data ?? "";
  const paths =
    current && !folders.data.includes(current)
      ? [...folders.data, current].sort((a, b) => a.localeCompare(b))
      : folders.data;
  const filtered = paths.filter((path) =>
    path.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  const normalized = normalizeFolderPath(query);
  const create = normalized && !paths.includes(normalized) ? normalized : null;
  const mutation = useMutation({
    mutationFn: (path: string) => saveSessionFolder(sessionId, path),
    onSuccess: () => {
      Keyboard.dismiss();
      onClose();
    },
  });
  const loading = folders.isLoading || assigned.isLoading;
  const failed = folders.error || assigned.error;
  const disabled = loading || Boolean(failed) || mutation.isPending;
  const select = (path: string) =>
    mutation.mutate(path === current ? "" : path);

  return (
    <View style={styles.content}>
      <View style={styles.header}>
        <Text accessibilityRole="header" style={styles.title}>
          Folder
        </Text>
        <IconButton
          accessibilityLabel="Close folder picker"
          icon="close"
          onPress={onClose}
        />
      </View>
      <TextInput
        accessibilityLabel={translate("Search or create folder", language)}
        placeholder={translate("Search or create folder", language)}
        placeholderTextColor={Colors.muted}
        value={query}
        onChangeText={setQuery}
        style={styles.search}
        autoCorrect={false}
        returnKeyType="done"
        onSubmitEditing={() => {
          if (create && !disabled) select(create);
        }}
      />
      {loading && <ActivityIndicator color={Colors.muted} />}
      {(failed || mutation.error) && (
        <Text accessibilityRole="alert" style={styles.error}>
          {mutation.error
            ? "Couldn’t move this note. Try again."
            : "Couldn’t load folders. Close and try again."}
        </Text>
      )}
      <FlatList
        data={filtered}
        keyExtractor={(path) => path}
        keyboardShouldPersistTaps="handled"
        renderItem={({ item: path }) => (
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={path}
            accessibilityState={{ selected: path === current, disabled }}
            disabled={disabled}
            onPress={() => select(path)}
            style={({ pressed }) => [styles.row, pressed && styles.pressed]}
          >
            <Ionicons name="folder-outline" size={20} color={Colors.ink} />
            <Text numberOfLines={1} style={styles.label}>
              {path}
            </Text>
            {path === current && (
              <Ionicons name="checkmark" size={20} color={Colors.ink} />
            )}
          </Pressable>
        )}
        ListEmptyComponent={
          !loading && !create ? (
            <Text style={styles.empty}>
              {normalized === null
                ? "Enter a valid folder name."
                : query.trim()
                  ? "No folders found."
                  : "No folders yet. Enter a name to create one."}
            </Text>
          ) : null
        }
        ListFooterComponent={
          create ? (
            <Pressable
              accessibilityRole="button"
              disabled={disabled}
              onPress={() => select(create)}
              style={({ pressed }) => [styles.row, pressed && styles.pressed]}
            >
              <Ionicons name="add" size={20} color={Colors.ink} />
              <Text style={styles.label}>Create “{create}”</Text>
            </Pressable>
          ) : null
        }
      />
      {mutation.isPending && <ActivityIndicator color={Colors.muted} />}
      {current ? (
        <Text style={styles.hint}>
          Tap the selected folder to remove this note from it.
        </Text>
      ) : null}
    </View>
  );
}

const useStyles = createStyleHook((Colors) => ({
  content: { flex: 1, gap: Spacing.sm, paddingBottom: Spacing.md },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: { ...Typography.section, color: Colors.ink },
  search: {
    ...Typography.body,
    color: Colors.ink,
    backgroundColor: Colors.surface,
    borderRadius: Radius.control,
    padding: Spacing.md,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.md,
    minHeight: 52,
    paddingHorizontal: Spacing.sm,
    borderRadius: Radius.control,
  },
  pressed: { backgroundColor: Colors.accentSurface },
  label: { flex: 1, ...Typography.body, color: Colors.ink },
  empty: {
    ...Typography.body,
    color: Colors.muted,
    paddingVertical: Spacing.md,
  },
  hint: { ...Typography.caption, color: Colors.muted },
  error: { ...Typography.caption, color: Colors.destructive },
}));
