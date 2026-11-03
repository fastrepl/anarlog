import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  StyleSheet,
  Text as NativeText,
  TextInput,
  View,
} from "react-native";

import { Radius, Spacing, Typography } from "@/constants/theme";
import {
  parseMeetingGlossaryText,
  readMeetingGlossary,
  saveMeetingGlossary,
} from "@/data/meeting-glossary";
import { Text, translate, useLanguage } from "@/i18n";
import { createStyleHook, useColors } from "@/settings/theme-provider";

export function MeetingGlossarySheet({
  sessionId,
  visible,
  onClose,
}: {
  sessionId: string;
  visible: boolean;
  onClose: () => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const glossary = useQuery({
    queryKey: ["meeting-glossary", sessionId],
    queryFn: () => readMeetingGlossary(sessionId),
    enabled: visible,
  });

  return (
    <Modal
      animationType="slide"
      onRequestClose={onClose}
      presentationStyle="pageSheet"
      visible={visible}
    >
      <KeyboardAvoidingView
        behavior={Platform.OS === "ios" ? "padding" : undefined}
        style={styles.container}
      >
        <View style={styles.header}>
          <Text accessibilityRole="header" style={styles.title}>
            Meeting glossary
          </Text>
          <Pressable
            accessibilityRole="button"
            onPress={onClose}
            style={({ pressed }) => [
              styles.headerButton,
              pressed && styles.pressed,
            ]}
          >
            <Text style={styles.headerButtonText}>Close</Text>
          </Pressable>
        </View>
        <Text style={styles.description}>
          Add names, products, and technical terms for this meeting, one per
          line. Transcription uses these as recognition hints; it does not
          replace words in the transcript.
        </Text>
        {glossary.isLoading ? (
          <ActivityIndicator color={Colors.muted} />
        ) : glossary.error ? (
          <Text accessibilityRole="alert" style={styles.error}>
            Couldn’t load this meeting’s glossary. Close and try again.
          </Text>
        ) : (
          <GlossaryEditor
            key={`${sessionId}:${glossary.data?.join("\n") ?? ""}`}
            sessionId={sessionId}
            initialTerms={glossary.data ?? []}
            onClose={onClose}
          />
        )}
      </KeyboardAvoidingView>
    </Modal>
  );
}

function GlossaryEditor({
  sessionId,
  initialTerms,
  onClose,
}: {
  sessionId: string;
  initialTerms: string[];
  onClose: () => void;
}) {
  const { language } = useLanguage();
  const styles = useStyles();
  const Colors = useColors();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState(initialTerms.join("\n"));
  const save = useMutation({
    mutationFn: () =>
      saveMeetingGlossary(sessionId, parseMeetingGlossaryText(draft)),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["meeting-glossary", sessionId],
      });
      onClose();
    },
  });

  return (
    <>
      <TextInput
        accessibilityLabel={translate("Meeting glossary terms", language)}
        autoCapitalize="sentences"
        autoCorrect={false}
        multiline
        onChangeText={setDraft}
        placeholder={"Cloudflare\nPostgreSQL\nstaging"}
        placeholderTextColor={Colors.muted}
        textAlignVertical="top"
        value={draft}
        style={styles.input}
      />
      {save.error && (
        <Text accessibilityRole="alert" style={styles.error}>
          {save.error instanceof Error
            ? save.error.message
            : "Couldn’t save this glossary."}
        </Text>
      )}
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ disabled: save.isPending }}
        disabled={save.isPending}
        onPress={() => save.mutate()}
        style={({ pressed }) => [
          styles.saveButton,
          { backgroundColor: Colors.accent },
          pressed && styles.pressed,
        ]}
      >
        <Text style={styles.saveButtonText}>
          {save.isPending ? "Saving…" : "Save glossary"}
        </Text>
      </Pressable>
    </>
  );
}

const useStyles = createStyleHook((Colors) => ({
  container: {
    flex: 1,
    padding: Spacing.lg,
    paddingTop: Spacing.xl,
    gap: Spacing.md,
    backgroundColor: Colors.background,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
  },
  title: { ...Typography.title, color: Colors.ink },
  headerButton: { padding: Spacing.sm, borderRadius: Radius.control },
  headerButtonText: { ...Typography.bodyStrong, color: Colors.accent },
  description: { ...Typography.body, color: Colors.muted },
  input: {
    minHeight: 180,
    maxHeight: 360,
    padding: Spacing.md,
    borderWidth: StyleSheet.hairlineWidth,
    borderColor: Colors.border,
    borderRadius: Radius.control,
    color: Colors.ink,
    ...Typography.body,
  },
  error: { ...Typography.caption, color: Colors.destructive },
  saveButton: {
    minHeight: 48,
    alignItems: "center",
    justifyContent: "center",
    borderRadius: Radius.control,
    paddingHorizontal: Spacing.md,
  },
  saveButtonText: { ...Typography.bodyStrong, color: Colors.background },
  pressed: { opacity: 0.72 },
}));
