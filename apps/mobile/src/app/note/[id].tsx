import { Ionicons } from "@expo/vector-icons";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useState } from "react";
import { Pressable, StyleSheet, TextInput, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { ListeningSheet } from "@/components/listening-sheet";
import { Colors, Spacing } from "@/constants/theme";
import { getSession } from "@/data/sessions";

export default function NoteScreen() {
  const router = useRouter();
  const { id } = useLocalSearchParams<{ id: string }>();
  const session = id === "new" ? undefined : getSession(id);
  const [listening, setListening] = useState(id === "new");

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <Pressable hitSlop={8} onPress={() => router.back()}>
          <Ionicons name="arrow-back" size={24} color={Colors.ink} />
        </Pressable>
        <Pressable hitSlop={8}>
          <Ionicons name="ellipsis-horizontal" size={22} color={Colors.ink} />
        </Pressable>
      </View>

      <TextInput
        style={styles.title}
        defaultValue={session?.title}
        placeholder="Untitled"
        placeholderTextColor={Colors.muted}
      />
      <TextInput
        style={styles.body}
        multiline
        placeholder="Start typing…"
        placeholderTextColor={Colors.muted}
        textAlignVertical="top"
      />

      {listening && <ListeningSheet onStop={() => setListening(false)} />}
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
  title: {
    paddingHorizontal: Spacing.lg,
    fontSize: 24,
    fontWeight: "700",
    color: Colors.ink,
  },
  body: {
    flex: 1,
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.md,
    fontSize: 16,
    color: Colors.ink,
  },
});
