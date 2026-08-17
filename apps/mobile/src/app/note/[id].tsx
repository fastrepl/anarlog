import { Ionicons } from "@expo/vector-icons";
import { File, Paths } from "expo-file-system";
import * as Linking from "expo-linking";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useRef, useState } from "react";
import {
  InputAccessoryView,
  Keyboard,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useSessionRecorder } from "@/audio/use-session-recorder";
import { AudioChip } from "@/components/audio-chip";
import { EditorAccessory } from "@/components/editor-accessory";
import { HandoffCard } from "@/components/handoff-card";
import { ListeningSheet } from "@/components/listening-sheet";
import { RemoteAudioCard } from "@/components/remote-audio-card";
import { Card } from "@/components/ui/card";
import { IconButton } from "@/components/ui/icon-button";
import { Colors, Spacing, Typography } from "@/constants/theme";
import { useSessionAudio } from "@/data/audio-catalog";
import { restoreSessionAudioFromPicker } from "@/data/restore-session-audio";
import {
  deleteSession,
  saveSessionNote,
  saveSessionTitle,
  useSessionDetail,
} from "@/data/session";
import { transcribeSession, useTranscriptionState } from "@/data/transcribe";
import { useSessionTranscripts } from "@/data/transcripts";
import { captureAnalytics } from "@/lib/analytics";
import { confirmDestructive } from "@/lib/confirm";
import { applyEditorFormat, type EditorFormat } from "@/lib/editor-format";
import { captureOperationalError } from "@/lib/error-reporting";
import { useMountEffect } from "@/lib/use-mount-effect";

function BodyEditor({
  accessoryId,
  defaultBodyFormat,
  defaultValue,
  editable,
  onChangeText,
}: {
  accessoryId: string;
  defaultBodyFormat: "prosemirror_json" | "markdown";
  defaultValue: string;
  editable: boolean;
  onChangeText: (
    body: string,
    bodyFormat: "prosemirror_json" | "markdown",
  ) => void;
}) {
  const inputRef = useRef<TextInput>(null);
  const textRef = useRef(defaultValue);
  const bodyFormatRef = useRef(defaultBodyFormat);
  const selectionRef = useRef({ start: 0, end: 0 });
  // Normal typing stays native so iOS retains its caret and scroll state;
  // toolbar commands briefly override both without remounting the editor.
  const [nativeOverride, setNativeOverride] = useState<{
    text: string;
    selection: { start: number; end: number };
  }>();
  const [androidKeyboardVisible, setAndroidKeyboardVisible] = useState(false);

  useMountEffect(() => {
    if (Platform.OS !== "android") return;
    const showSubscription = Keyboard.addListener("keyboardDidShow", () =>
      setAndroidKeyboardVisible(true),
    );
    const hideSubscription = Keyboard.addListener("keyboardDidHide", () =>
      setAndroidKeyboardVisible(false),
    );
    return () => {
      showSubscription.remove();
      hideSubscription.remove();
    };
  });

  const handleChangeText = (body: string) => {
    textRef.current = body;
    onChangeText(body, bodyFormatRef.current);
  };

  const handleFormat = (format: EditorFormat) => {
    const formatted = applyEditorFormat(
      textRef.current,
      selectionRef.current,
      format,
    );
    textRef.current = formatted.text;
    bodyFormatRef.current = formatted.bodyFormat;
    selectionRef.current = formatted.selection;
    setNativeOverride({
      text: formatted.text,
      selection: formatted.selection,
    });
    onChangeText(formatted.text, formatted.bodyFormat);
    inputRef.current?.focus();
    requestAnimationFrame(() => setNativeOverride(undefined));
  };

  const handleDismissKeyboard = () => {
    inputRef.current?.blur();
    Keyboard.dismiss();
  };

  return (
    <>
      <TextInput
        ref={inputRef}
        style={styles.body}
        multiline
        editable={editable}
        inputAccessoryViewID={Platform.OS === "ios" ? accessoryId : undefined}
        defaultValue={defaultValue}
        value={nativeOverride?.text}
        selection={nativeOverride?.selection}
        placeholder="Start typing…"
        placeholderTextColor={Colors.muted}
        textAlignVertical="top"
        onChangeText={handleChangeText}
        onSelectionChange={(event) => {
          selectionRef.current = event.nativeEvent.selection;
        }}
      />
      {Platform.OS === "ios" && editable && (
        <InputAccessoryView
          nativeID={accessoryId}
          backgroundColor={Colors.background}
        >
          <EditorAccessory
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </InputAccessoryView>
      )}
      {Platform.OS === "android" && editable && androidKeyboardVisible && (
        <View style={styles.androidAccessory}>
          <EditorAccessory
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </View>
      )}
    </>
  );
}

export default function NoteScreen() {
  const router = useRouter();
  const { id, listen } = useLocalSearchParams<{
    id: string;
    listen?: string;
  }>();
  const { data, isLoading } = useSessionDetail(id);
  const audio = useSessionAudio(id);
  const transcripts = useSessionTranscripts(id);
  const transcription = useTranscriptionState(id);
  const [listening, setListening] = useState(listen === "1");
  const recorder = useSessionRecorder(id, listening);
  const [audioRestoreError, setAudioRestoreError] = useState<string | null>(
    null,
  );
  const [restoringAudio, setRestoringAudio] = useState(false);
  const audioRestoreBusyRef = useRef(false);
  const localAudioFile = audio.data?.localRelativePath
    ? new File(Paths.document, "sessions", id, audio.data.localRelativePath)
    : null;
  const localAudioAvailable =
    audio.data?.availableLocally === true && localAudioFile?.exists === true;

  const dataRef = useRef(data);
  dataRef.current = data;
  const draftRef = useRef<{
    title?: string;
    body?: string;
    bodyFormat?: "prosemirror_json" | "markdown";
  }>({});
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // The live query lags our own writes, so a body-only flush would otherwise
  // resend the pre-edit title and undo the title we just persisted.
  const savedTitleRef = useRef<string | null>(null);
  if (savedTitleRef.current !== null && data?.title === savedTitleRef.current) {
    savedTitleRef.current = null;
  }

  const flush = () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const draft = draftRef.current;
    draftRef.current = {};
    const current = dataRef.current;
    if (!current) return;
    if (draft.body !== undefined) {
      const title = draft.title ?? savedTitleRef.current ?? current.title;
      const bodyFormat = draft.bodyFormat ?? current.bodyFormat;
      savedTitleRef.current = title;
      void saveSessionNote(id, {
        title,
        bodyText: draft.body,
        bodyFormat,
      }).catch((error) => {
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: {
            body_format: bodyFormat,
            edit_type: "body",
          },
        });
      });
    } else if (draft.title !== undefined) {
      savedTitleRef.current = draft.title;
      void saveSessionTitle(id, draft.title).catch((error) => {
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: { edit_type: "title" },
        });
      });
    }
  };

  useMountEffect(() => {
    captureAnalytics("note_opened", {
      entry_point: "mobile_note",
    });
    return flush;
  });

  const onEdit = (
    patch: Partial<{
      title: string;
      body: string;
      bodyFormat: "prosemirror_json" | "markdown";
    }>,
  ) => {
    draftRef.current = { ...draftRef.current, ...patch };
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(flush, 500);
  };

  const handleBack = async () => {
    await recorder.stop();
    flush();
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  const handleStop = async () => {
    const result = await recorder.stop();
    if (result !== "failed") setListening(false);
  };

  const handleRetryRecording = async () => {
    const result = await recorder.retry();
    if (result === "saved") setListening(false);
  };

  const handleOpenSettings = async () => {
    try {
      await Linking.openSettings();
    } catch (error) {
      captureOperationalError(error, {
        operation: "recording_permission_settings_open",
      });
    }
  };

  const handleChooseRecording = async () => {
    if (audioRestoreBusyRef.current || !audio.data) return;
    audioRestoreBusyRef.current = true;
    setRestoringAudio(true);
    setAudioRestoreError(null);
    try {
      const result = await restoreSessionAudioFromPicker(id, audio.data);
      if (result === "restored") {
        captureAnalytics("file_uploaded", {
          entry_point: "mobile_audio_restore",
          file_type: "audio",
          content_type: audio.data.contentType,
          size_bytes: audio.data.sizeBytes,
        });
      }
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_audio_restore",
      });
      setAudioRestoreError(
        error instanceof Error
          ? error.message
          : "The recording could not be added to this phone.",
      );
    } finally {
      audioRestoreBusyRef.current = false;
      setRestoringAudio(false);
    }
  };

  const handleDelete = async () => {
    const confirmed = await confirmDestructive(
      `Delete "${data?.title || "Untitled"}"?`,
      "Delete",
    );
    if (!confirmed) return;
    await recorder.stop();
    draftRef.current = {};
    try {
      await deleteSession(id);
      if (router.canGoBack()) router.back();
      else router.replace("/");
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_delete",
        tags: { entry_point: "mobile_note" },
      });
    }
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="arrow-back"
          iconSize={22}
          onPress={() => void handleBack()}
        />
        <IconButton
          accessibilityLabel="Delete note"
          icon="trash-outline"
          onPress={() => void handleDelete()}
          tone="muted"
        />
      </View>

      {!isLoading && data && (
        <View key={data.id} style={styles.editor}>
          <TextInput
            style={styles.title}
            defaultValue={data.title}
            placeholder="Untitled"
            placeholderTextColor={Colors.muted}
            onChangeText={(title) => onEdit({ title })}
          />
          {data.summary && (
            <Card style={styles.summary} tone="muted">
              <Text style={styles.summaryLabel}>Summary</Text>
              {data.summary.title !== "Summary" && (
                <Text style={styles.summaryTitle}>{data.summary.title}</Text>
              )}
              {data.summary.text !== "" && (
                <ScrollView style={styles.summaryScroll} nestedScrollEnabled>
                  <Text style={styles.summaryText}>{data.summary.text}</Text>
                </ScrollView>
              )}
            </Card>
          )}
          {audio.data && localAudioAvailable && localAudioFile && (
            <View key={`${audio.data.filename}:${audio.data.createdAt}`}>
              <AudioChip
                uri={localAudioFile.uri}
                filename={audio.data.filename}
                sizeBytes={audio.data.sizeBytes}
              />
              <HandoffCard
                uri={localAudioFile.uri}
                filename={audio.data.filename}
              />
            </View>
          )}
          {audio.data && !localAudioAvailable && (
            <RemoteAudioCard
              errorMessage={audioRestoreError}
              loading={restoringAudio}
              onChooseRecording={() => void handleChooseRecording()}
            />
          )}
          {audio.data &&
            localAudioAvailable &&
            audio.data.transcriptStatus !== "complete" &&
            transcripts.length === 0 &&
            (transcription === "running" ? (
              <Text style={styles.transcribeStatus}>Transcribing…</Text>
            ) : (
              <Pressable
                hitSlop={4}
                onPress={() => void transcribeSession(id)}
                style={({ pressed }) => pressed && styles.transcribePressed}
              >
                <Text style={styles.transcribeAction}>
                  {transcription === "failed"
                    ? "Transcription failed — tap to retry"
                    : "Tap to transcribe"}
                </Text>
              </Pressable>
            ))}
          {transcripts.length > 0 && (
            <Card style={styles.transcript} tone="muted">
              <Text style={styles.transcriptTitle}>Transcript</Text>
              <ScrollView style={styles.transcriptScroll} nestedScrollEnabled>
                {transcripts.map((segment) => (
                  <Text key={segment.id} style={styles.transcriptText}>
                    {segment.text}
                  </Text>
                ))}
              </ScrollView>
            </Card>
          )}
          <Text style={styles.noteLabel}>Notes</Text>
          {!data.plainEditable && (
            <View style={styles.readOnlyChip}>
              <Ionicons
                name="lock-closed-outline"
                size={12}
                color={Colors.muted}
              />
              <Text style={styles.readOnlyLabel}>
                Formatted note — edit the body on desktop
              </Text>
            </View>
          )}
          <BodyEditor
            accessoryId={`note-editor-controls-${data.id}`}
            defaultBodyFormat={data.bodyFormat}
            defaultValue={data.noteText}
            editable={data.plainEditable}
            onChangeText={(body, bodyFormat) => onEdit({ body, bodyFormat })}
          />
        </View>
      )}

      {listening && (
        <ListeningSheet
          phase={recorder.phase}
          failure={recorder.failure}
          amplitude={recorder.amplitude}
          durationMs={recorder.durationMs}
          liveStatus={recorder.liveStatus}
          liveTranscript={recorder.liveTranscript}
          onStop={() => void handleStop()}
          onRetry={() => void handleRetryRecording()}
          onOpenSettings={() => void handleOpenSettings()}
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
    paddingVertical: Spacing.md,
  },
  editor: {
    flex: 1,
  },
  title: {
    paddingHorizontal: Spacing.lg,
    ...Typography.title,
    color: Colors.ink,
  },
  summary: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.md,
    padding: Spacing.md,
  },
  summaryLabel: {
    ...Typography.captionStrong,
    color: Colors.muted,
  },
  summaryTitle: {
    marginTop: Spacing.xs,
    ...Typography.section,
    color: Colors.ink,
  },
  summaryScroll: {
    maxHeight: 220,
    marginTop: Spacing.sm,
  },
  summaryText: {
    ...Typography.body,
    color: Colors.ink,
  },
  transcribeStatus: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  transcribeAction: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.xs,
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  transcribePressed: {
    opacity: 0.6,
  },
  transcript: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
    paddingHorizontal: Spacing.md,
    paddingTop: Spacing.sm,
    paddingBottom: Spacing.xs,
  },
  transcriptTitle: {
    ...Typography.captionStrong,
    color: Colors.muted,
    marginBottom: Spacing.xs,
  },
  transcriptScroll: {
    maxHeight: 160,
  },
  transcriptText: {
    ...Typography.body,
    color: Colors.ink,
    marginBottom: Spacing.sm,
  },
  noteLabel: {
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.md,
    ...Typography.captionStrong,
    color: Colors.muted,
  },
  readOnlyChip: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginHorizontal: Spacing.lg,
    marginTop: Spacing.sm,
  },
  readOnlyLabel: {
    ...Typography.caption,
    color: Colors.muted,
  },
  body: {
    flex: 1,
    paddingHorizontal: Spacing.lg,
    paddingTop: Spacing.md,
    ...Typography.body,
    color: Colors.ink,
  },
  androidAccessory: {
    backgroundColor: Colors.background,
  },
});
