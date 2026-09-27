import SegmentedControl from "@expo/ui/community/segmented-control";
import { Ionicons } from "@expo/vector-icons";
import { File, Paths } from "expo-file-system";
import * as Linking from "expo-linking";
import { useLocalSearchParams, useRouter } from "expo-router";
import { useRef, useState } from "react";
import {
  Alert,
  InputAccessoryView,
  Keyboard,
  Platform,
  Pressable,
  ScrollView,
  Share,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useSessionRecorder } from "@/audio/use-session-recorder";
import { useAuth } from "@/auth/context";
import { AudioChip } from "@/components/audio-chip";
import { EditorAccessory } from "@/components/editor-accessory";
import { FolderPickerSheet } from "@/components/folder-picker-sheet";
import { ListeningSheet } from "@/components/listening-sheet";
import { MarkdownView } from "@/components/markdown-view";
import { NoteActionsSheet } from "@/components/note-actions-sheet";
import { NoteAttachmentCard } from "@/components/note-attachment-card";
import { NoteConflictBanner } from "@/components/note-conflict-banner";
import { RecordingSyncCard } from "@/components/recording-sync-card";
import { RemoteAudioCard } from "@/components/remote-audio-card";
import { SessionTranscript } from "@/components/session-transcript";
import { StartListeningButton } from "@/components/start-listening-button";
import { Button } from "@/components/ui/button";
import { IconButton } from "@/components/ui/icon-button";
import { VersionHistorySheet } from "@/components/version-history-sheet";
import { Spacing, Typography } from "@/constants/theme";
import { useSessionAudio } from "@/data/audio-catalog";
import type { RestoredNote } from "@/data/conflicts";
import { importRecordingIntoSession } from "@/data/import-voice-memo";
import {
  type NoteAttachment,
  useNoteAttachments,
} from "@/data/note-attachment-catalog";
import { insertCapturedNoteAttachmentMarkdown } from "@/data/note-attachment-model";
import { pickAndCatalogNoteAttachment } from "@/data/note-attachments";
import {
  restoreNoteAttachmentFromCloud,
  shareNoteAttachment,
} from "@/data/restore-note-attachment";
import {
  restoreSessionAudioFromCloud,
  restoreSessionAudioFromPicker,
} from "@/data/restore-session-audio";
import {
  deleteSession,
  saveSessionNote,
  saveSessionTitle,
  useSessionDetail,
} from "@/data/session";
import { sessionView, type SessionViewSelection } from "@/data/session-view";
import {
  summarizeSession,
  useAutomaticSummary,
  useSessionSummaryState,
} from "@/data/summarize";
import { transcribeSession, useTranscriptionState } from "@/data/transcribe";
import {
  loadSessionTranscripts,
  useSessionHasTranscript,
} from "@/data/transcripts";
import { captureAnalytics } from "@/lib/analytics";
import { confirmDestructive } from "@/lib/confirm";
import { applyEditorFormat, type EditorFormat } from "@/lib/editor-format";
import { env } from "@/lib/env";
import { captureOperationalError } from "@/lib/error-reporting";
import { useKeyboardVisible } from "@/lib/use-keyboard-visible";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook, useColors } from "@/settings/theme-provider";
import { useProviderAccess } from "@/settings/use-provider-access";

function BodyEditor({
  accessoryId,
  defaultBodyFormat,
  defaultValue,
  editable,
  onAttach,
  onChangeText,
  onCommit,
  onFocusChange,
}: {
  accessoryId: string;
  defaultBodyFormat: "prosemirror_json" | "markdown";
  defaultValue: string;
  editable: boolean;
  onAttach: (signal: AbortSignal) => Promise<{ markdown: string } | null>;
  onChangeText: (
    body: string,
    bodyFormat: "prosemirror_json" | "markdown",
  ) => void;
  onCommit: () => void;
  onFocusChange: (focused: boolean) => void;
}) {
  const styles = useStyles();
  const Colors = useColors();
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
  const [attaching, setAttaching] = useState(false);
  const attachControllerRef = useRef<AbortController | null>(null);
  const editorActiveRef = useRef(true);

  useMountEffect(() => {
    editorActiveRef.current = true;
    return () => {
      editorActiveRef.current = false;
      attachControllerRef.current?.abort();
    };
  });

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
    if (attaching) return;
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

  const handleAttach = async () => {
    if (attachControllerRef.current) return;
    const controller = new AbortController();
    const capturedText = textRef.current;
    const capturedSelection = { ...selectionRef.current };
    attachControllerRef.current = controller;
    setAttaching(true);
    try {
      const attachment = await onAttach(controller.signal);
      if (
        !attachment ||
        controller.signal.aborted ||
        !editorActiveRef.current
      ) {
        return;
      }
      const inserted = insertCapturedNoteAttachmentMarkdown({
        capturedText,
        capturedSelection,
        currentText: textRef.current,
        markdown: attachment.markdown,
      });
      textRef.current = inserted.text;
      bodyFormatRef.current = "markdown";
      selectionRef.current = inserted.selection;
      setNativeOverride(inserted);
      onChangeText(inserted.text, "markdown");
      onCommit();
      inputRef.current?.focus();
      requestAnimationFrame(() => {
        if (editorActiveRef.current) setNativeOverride(undefined);
      });
    } finally {
      if (attachControllerRef.current === controller) {
        attachControllerRef.current = null;
      }
      if (editorActiveRef.current) setAttaching(false);
    }
  };

  return (
    <>
      <TextInput
        ref={inputRef}
        style={styles.body}
        multiline
        editable={editable && !attaching}
        inputAccessoryViewID={Platform.OS === "ios" ? accessoryId : undefined}
        defaultValue={defaultValue}
        value={nativeOverride?.text}
        selection={nativeOverride?.selection}
        placeholder="Start typing…"
        placeholderTextColor={Colors.muted}
        textAlignVertical="top"
        onChangeText={handleChangeText}
        onBlur={() => onFocusChange(false)}
        onFocus={() => onFocusChange(true)}
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
            attaching={attaching}
            onAttach={() => void handleAttach()}
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </InputAccessoryView>
      )}
      {Platform.OS === "android" && editable && androidKeyboardVisible && (
        <View style={styles.androidAccessory}>
          <EditorAccessory
            attaching={attaching}
            onAttach={() => void handleAttach()}
            onFormat={handleFormat}
            onDismiss={handleDismissKeyboard}
          />
        </View>
      )}
    </>
  );
}

export default function NoteScreen() {
  const styles = useStyles();
  const Colors = useColors();
  const router = useRouter();
  const auth = useAuth();
  const canTranscribe = useProviderAccess("stt");
  const canSummarize = useProviderAccess("llm");
  const { id, listen } = useLocalSearchParams<{
    id: string;
    listen?: string;
  }>();
  const { data, isLoading } = useSessionDetail(id);
  const audio = useSessionAudio(id);
  const noteAttachments = useNoteAttachments(id);
  const transcriptState = useSessionHasTranscript(id);
  const summaryState = useSessionSummaryState(id);
  const [selection, setSelection] = useState<SessionViewSelection | null>(null);
  const transcription = useTranscriptionState(id);
  const [listening, setListening] = useState(listen === "1");
  const [editorFocused, setEditorFocused] = useState(false);
  const [actionsOpen, setActionsOpen] = useState(false);
  const [foldersOpen, setFoldersOpen] = useState(false);
  const [versionsOpen, setVersionsOpen] = useState(false);
  // A restore writes the note behind the uncontrolled inputs, so they are
  // remounted with what was just written instead of waiting for the live
  // query. A title-only restore leaves the body editor alone.
  const [restored, setRestored] = useState<
    (RestoredNote & { titleToken: number; bodyToken: number }) | null
  >(null);
  const keyboardVisible = useKeyboardVisible();
  const recorder = useSessionRecorder(id, listening);
  const [audioRestoreError, setAudioRestoreError] = useState<string | null>(
    null,
  );
  const [restoringAudio, setRestoringAudio] = useState(false);
  const audioRestoreBusyRef = useRef(false);
  const audioRestoreControllerRef = useRef<AbortController | null>(null);
  const attachmentActionBusyRef = useRef(false);
  const attachmentRestoreControllerRef = useRef<AbortController | null>(null);
  const [attachmentActionId, setAttachmentActionId] = useState<string | null>(
    null,
  );
  const [attachmentActionError, setAttachmentActionError] = useState<{
    attachmentId: string;
    message: string;
  } | null>(null);
  const screenActiveRef = useRef(true);
  const localAudioFile = audio.data?.localRelativePath
    ? new File(Paths.document, "sessions", id, audio.data.localRelativePath)
    : null;
  const localAudioAvailable =
    audio.data?.availableLocally === true && localAudioFile?.exists === true;
  const hasRecordingHistory =
    Boolean(audio.data) || transcriptState.data === true;
  const active = listening && recorder.phase !== "saved";
  const { tabs, current, selectedIndex } = sessionView({
    sessionId: id,
    active,
    hasRecordingHistory,
    hasSummary: Boolean(data?.summary),
    hasTranscript: transcriptState.data === true,
    selection,
  });
  const showMemos = current.type === "raw";
  const automaticSummary = useAutomaticSummary(
    id,
    !active &&
      !isLoading &&
      Boolean(data) &&
      !data?.summary &&
      canSummarize &&
      audio.data?.transcriptStatus === "complete" &&
      transcriptState.data === true &&
      transcription !== "running" &&
      summaryState?.status !== "error",
  );
  const summaryPending =
    summaryState?.status === "pending" || automaticSummary.isFetching;
  const summaryError = summaryState?.error;
  const needsTranscription =
    Boolean(audio.data) && audio.data?.transcriptStatus !== "complete";
  const localNoteAttachments = noteAttachments.map((attachment) => {
    const file = attachment.localRelativePath
      ? new File(Paths.document, "sessions", id, attachment.localRelativePath)
      : null;
    return {
      attachment,
      file: file?.exists === true ? file : null,
    };
  });

  const dataRef = useRef(data);
  dataRef.current = data;
  const draftRef = useRef<{
    title?: string;
    body?: string;
    bodyFormat?: "prosemirror_json" | "markdown";
  }>({});
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const showEmptyNoteCta =
    !active &&
    !editorFocused &&
    !audio.isLoading &&
    !transcriptState.isLoading &&
    !transcriptState.error &&
    data !== null &&
    data.title.trim() === "" &&
    data.noteText.trim() === "" &&
    !data.summary &&
    !hasRecordingHistory &&
    noteAttachments.length === 0;

  // The live query lags our own writes, so a body-only flush would otherwise
  // resend the pre-edit title and undo the title we just persisted.
  const savedTitleRef = useRef<string | null>(null);
  if (savedTitleRef.current !== null && data?.title === savedTitleRef.current) {
    savedTitleRef.current = null;
  }

  const flush = (throwOnError = false) => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const draft = draftRef.current;
    draftRef.current = {};
    const restoreDraft = () => {
      draftRef.current = { ...draft, ...draftRef.current };
    };
    const current = dataRef.current;
    if (!current) return;
    if (draft.body !== undefined) {
      const title = draft.title ?? savedTitleRef.current ?? current.title;
      const bodyFormat = draft.bodyFormat ?? current.bodyFormat;
      savedTitleRef.current = title;
      return saveSessionNote(id, {
        title,
        bodyText: draft.body,
        bodyFormat,
      }).catch((error) => {
        restoreDraft();
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: {
            body_format: bodyFormat,
            edit_type: "body",
          },
        });
        if (throwOnError) throw error;
      });
    } else if (draft.title !== undefined) {
      savedTitleRef.current = draft.title;
      return saveSessionTitle(id, draft.title).catch((error) => {
        restoreDraft();
        captureOperationalError(error, {
          operation: "session_note_save",
          tags: { edit_type: "title" },
        });
        if (throwOnError) throw error;
      });
    }
  };

  const handleRestored = (note: RestoredNote) => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    draftRef.current = {};
    savedTitleRef.current = note.title;
    setRestored((current) => ({
      ...note,
      titleToken: (current?.titleToken ?? 0) + 1,
      bodyToken:
        (current?.bodyToken ?? 0) + (note.bodyText === null ? 0 : 1),
    }));
  };

  useMountEffect(() => {
    screenActiveRef.current = true;
    captureAnalytics("note_opened", {
      entry_point: "mobile_note",
    });
    return () => {
      screenActiveRef.current = false;
      audioRestoreControllerRef.current?.abort();
      attachmentRestoreControllerRef.current?.abort();
      flush();
    };
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
    await flush();
    await recorder.stop();
    if (router.canGoBack()) router.back();
    else router.replace("/");
  };

  const handleStop = async () => {
    await flush();
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

  const handleImportRecording = async () => {
    if (audioRestoreBusyRef.current || hasRecordingHistory) return;
    audioRestoreBusyRef.current = true;
    try {
      await importRecordingIntoSession(id, auth.session?.user.id);
    } catch (error) {
      captureOperationalError(error, {
        operation: "voice_memo_import",
        tags: { entry_point: "mobile_note" },
      });
      Alert.alert(
        "Couldn’t import recording",
        error instanceof Error
          ? error.message
          : "The selected recording could not be imported.",
      );
    } finally {
      audioRestoreBusyRef.current = false;
    }
  };

  const handleDownloadRecording = async () => {
    const accessToken = auth.session?.access_token;
    if (
      audioRestoreBusyRef.current ||
      !audio.data?.cloudObjectKey ||
      !accessToken ||
      !env.supabaseUrl
    ) {
      return;
    }
    audioRestoreBusyRef.current = true;
    const controller = new AbortController();
    audioRestoreControllerRef.current = controller;
    setRestoringAudio(true);
    setAudioRestoreError(null);
    try {
      await restoreSessionAudioFromCloud(id, audio.data, {
        accessToken,
        apiBaseUrl: env.apiUrl,
        supabaseUrl: env.supabaseUrl,
        signal: controller.signal,
      });
      captureAnalytics("audio_restored", {
        entry_point: "cloud_sync",
        content_type: audio.data.contentType,
        size_bytes: audio.data.sizeBytes,
      });
    } catch (error) {
      if (!controller.signal.aborted && screenActiveRef.current) {
        captureOperationalError(error, {
          operation: "session_audio_cloud_restore",
        });
        setAudioRestoreError(
          error instanceof Error
            ? error.message
            : "The recording could not be downloaded to this phone.",
        );
      }
    } finally {
      if (audioRestoreControllerRef.current === controller) {
        audioRestoreControllerRef.current = null;
      }
      audioRestoreBusyRef.current = false;
      if (screenActiveRef.current) setRestoringAudio(false);
    }
  };

  const handleAttachFile = async (
    signal: AbortSignal,
  ): Promise<{ markdown: string } | null> => {
    try {
      const result = await pickAndCatalogNoteAttachment(id, signal);
      if (result.status === "cancelled") return null;
      captureAnalytics("file_uploaded", {
        entry_point: "mobile_note_attachment",
        file_type: "attachment",
      });
      return { markdown: result.markdown };
    } catch (error) {
      if (signal.aborted) return null;
      captureOperationalError(error, {
        operation: "note_attachment_import",
      });
      Alert.alert(
        "Couldn’t attach file",
        error instanceof Error
          ? error.message
          : "The selected file could not be attached.",
      );
      return null;
    }
  };

  const handleDownloadAttachment = async (attachment: NoteAttachment) => {
    const accessToken = auth.session?.access_token;
    if (
      attachmentActionBusyRef.current ||
      !attachment.cloudObjectKey ||
      !accessToken ||
      !env.supabaseUrl
    ) {
      return;
    }
    attachmentActionBusyRef.current = true;
    const controller = new AbortController();
    attachmentRestoreControllerRef.current = controller;
    setAttachmentActionId(attachment.attachmentId);
    setAttachmentActionError(null);
    try {
      await restoreNoteAttachmentFromCloud(id, attachment, {
        accessToken,
        apiBaseUrl: env.apiUrl,
        supabaseUrl: env.supabaseUrl,
        signal: controller.signal,
      });
      captureAnalytics("file_downloaded", {
        entry_point: "mobile_note_attachment",
        file_type: "attachment",
        size_bytes: attachment.sizeBytes,
      });
    } catch (error) {
      if (!controller.signal.aborted && screenActiveRef.current) {
        captureOperationalError(error, {
          operation: "note_attachment_cloud_restore",
        });
        setAttachmentActionError({
          attachmentId: attachment.attachmentId,
          message:
            error instanceof Error
              ? error.message
              : "The file could not be downloaded to this phone.",
        });
      }
    } finally {
      if (attachmentRestoreControllerRef.current === controller) {
        attachmentRestoreControllerRef.current = null;
      }
      attachmentActionBusyRef.current = false;
      if (screenActiveRef.current) setAttachmentActionId(null);
    }
  };

  const handleShareAttachment = async (
    attachment: NoteAttachment,
    uri: string,
  ) => {
    if (attachmentActionBusyRef.current) return;
    attachmentActionBusyRef.current = true;
    setAttachmentActionId(attachment.attachmentId);
    setAttachmentActionError(null);
    try {
      await shareNoteAttachment(uri, attachment);
      captureAnalytics("file_shared", {
        entry_point: "mobile_note_attachment",
        file_type: "attachment",
      });
    } catch (error) {
      captureOperationalError(error, {
        operation: "note_attachment_share",
      });
      if (screenActiveRef.current) {
        setAttachmentActionError({
          attachmentId: attachment.attachmentId,
          message:
            error instanceof Error
              ? error.message
              : "The file could not be shared.",
        });
      }
    } finally {
      attachmentActionBusyRef.current = false;
      if (screenActiveRef.current) setAttachmentActionId(null);
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

  const handleExport = async () => {
    const current = dataRef.current;
    if (!current) return;
    const draft = { ...draftRef.current };
    flush();

    const title = (draft.title ?? current.title).trim() || "Untitled";
    const note = (draft.body ?? current.noteText).trim();
    try {
      const transcripts = await loadSessionTranscripts(id);
      const transcript = transcripts
        .map((segment) => `${segment.speaker}: ${segment.text}`)
        .join("\n\n")
        .trim();
      const sections = [`# ${title}`];
      if (current.summary) {
        sections.push(
          `## ${current.summary.title}\n\n${current.summary.text}`.trim(),
        );
      }
      if (note) sections.push(`## Notes\n\n${note}`);
      if (transcript) sections.push(`## Transcript\n\n${transcript}`);

      await Share.share({
        title,
        message: sections.join("\n\n"),
      });
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_export",
        tags: { entry_point: "mobile_note" },
      });
    }
  };

  const handleListeningAction = () => {
    if (active) void handleStop();
    else if (
      !audio.isLoading &&
      !transcriptState.isLoading &&
      !transcriptState.error &&
      !hasRecordingHistory
    ) {
      setListening(true);
      void recorder.start();
    }
  };

  const handleMoreActions = () => {
    if (!data) return;
    Keyboard.dismiss();
    setEditorFocused(false);
    setActionsOpen(true);
  };

  return (
    <SafeAreaView style={styles.safeArea}>
      <View style={styles.header}>
        <IconButton
          accessibilityLabel="Back"
          icon="back"
          iconSize={22}
          onPress={() => void handleBack()}
        />
        {!isLoading && data ? (
          <TextInput
            key={`${data.id}:${restored?.titleToken ?? 0}`}
            accessibilityLabel="Note title"
            style={styles.title}
            defaultValue={restored?.title ?? data.title}
            placeholder="Untitled"
            placeholderTextColor={Colors.muted}
            returnKeyType="done"
            onSubmitEditing={() => Keyboard.dismiss()}
            onBlur={() => {
              setEditorFocused(false);
              void flush();
            }}
            onChangeText={(title) => onEdit({ title })}
            onFocus={() => setEditorFocused(true)}
          />
        ) : (
          <View style={styles.title} />
        )}
        <IconButton
          accessibilityLabel="More actions"
          disabled={!data}
          icon="more"
          iconSize={22}
          onPress={handleMoreActions}
          tone="muted"
        />
      </View>

      {!isLoading && data && (
        <View key={data.id} style={styles.editor}>
          {tabs.length > 1 && (
            <View style={styles.tabs}>
              <SegmentedControl
                values={tabs.map((tab) =>
                  tab.type === "enhanced"
                    ? "Summary"
                    : tab.type === "raw"
                      ? "Memos"
                      : "Transcript",
                )}
                selectedIndex={selectedIndex}
                onChange={(event) => {
                  void flush();
                  Keyboard.dismiss();
                  setEditorFocused(false);
                  const view = tabs[event.nativeEvent.selectedSegmentIndex];
                  if (view) setSelection({ sessionId: id, active, view });
                }}
              />
            </View>
          )}
          {current.type === "enhanced" && (
            <ScrollView
              style={styles.summaryScroll}
              contentContainerStyle={styles.summary}
            >
              {data.summary && (
                <View>
                  {data.summary.title !== "Summary" && (
                    <Text style={styles.summaryTitle}>
                      {data.summary.title}
                    </Text>
                  )}
                  <MarkdownView markdown={data.summary.text} />
                </View>
              )}
              {!data.summary && (
                <Text style={styles.summaryText}>
                  {active
                    ? "Your summary will be generated after the meeting."
                    : summaryPending
                      ? "Generating summary…"
                      : transcription === "running"
                        ? "Finishing transcription. Your summary will follow automatically."
                        : needsTranscription
                          ? "Your recording is saved. Finish transcription to get your summary."
                          : "Your meeting summary will appear here. Your memos are in the Memos tab."}
                </Text>
              )}
              {!active && needsTranscription && transcription !== "running" && (
                <>
                  {!localAudioAvailable && (
                    <RemoteAudioCard
                      cloudAvailable={Boolean(
                        audio.data?.cloudObjectKey &&
                        auth.billing.isPro &&
                        auth.session?.access_token &&
                        env.supabaseUrl,
                      )}
                      errorMessage={audioRestoreError}
                      loading={restoringAudio}
                      onDownloadRecording={() => void handleDownloadRecording()}
                      onChooseRecording={() => void handleChooseRecording()}
                    />
                  )}
                  {localAudioAvailable && (
                    <Button
                      label={
                        canTranscribe
                          ? "Retry transcription"
                          : "Choose transcription provider"
                      }
                      variant="ghost"
                      size="small"
                      onPress={() =>
                        canTranscribe
                          ? void transcribeSession(id)
                          : router.push("/settings/transcription-provider")
                      }
                    />
                  )}
                </>
              )}
              {summaryError && (
                <Text accessibilityRole="alert" style={styles.summaryError}>
                  {summaryError.message}
                </Text>
              )}
              {!active &&
                !needsTranscription &&
                (!canSummarize ||
                  summaryError ||
                  data.summary ||
                  (!audio.data && transcriptState.data)) && (
                  <Button
                    label={
                      !canSummarize
                        ? "Choose summary provider"
                        : summaryError
                          ? "Retry summary"
                          : data.summary
                            ? "Regenerate summary"
                            : "Generate summary"
                    }
                    loading={summaryPending}
                    disabled={transcription === "running"}
                    variant="ghost"
                    size="small"
                    onPress={() =>
                      canSummarize
                        ? void summarizeSession(id, {
                            beforeGenerate: () => flush(true),
                          }).catch(() => {})
                        : router.push("/settings/summary-provider")
                    }
                  />
                )}
            </ScrollView>
          )}
          {current.type === "transcript" && (
            <SessionTranscript
              sessionId={id}
              live={
                active
                  ? {
                      status: recorder.liveStatus,
                      text: recorder.liveTranscript,
                    }
                  : undefined
              }
              recordingDetails={
                !active && (
                  <>
                    {audio.data && localAudioAvailable && localAudioFile && (
                      <View
                        key={`${audio.data.filename}:${audio.data.createdAt}`}
                      >
                        <AudioChip
                          uri={localAudioFile.uri}
                          filename={audio.data.filename}
                          sizeBytes={audio.data.sizeBytes}
                        />
                        <RecordingSyncCard audio={audio.data} />
                      </View>
                    )}
                    {audio.data && !localAudioAvailable && (
                      <RemoteAudioCard
                        cloudAvailable={Boolean(
                          audio.data.cloudObjectKey &&
                          auth.billing.isPro &&
                          auth.session?.access_token &&
                          env.supabaseUrl,
                        )}
                        errorMessage={audioRestoreError}
                        loading={restoringAudio}
                        onDownloadRecording={() =>
                          void handleDownloadRecording()
                        }
                        onChooseRecording={() => void handleChooseRecording()}
                      />
                    )}
                    {audio.data &&
                      localAudioAvailable &&
                      audio.data.transcriptStatus !== "complete" &&
                      transcriptState.data === false &&
                      (transcription === "running" ? (
                        <Text style={styles.transcribeStatus}>
                          Transcribing…
                        </Text>
                      ) : (
                        <Pressable
                          hitSlop={4}
                          onPress={() =>
                            canTranscribe
                              ? void transcribeSession(id)
                              : router.push("/settings/transcription-provider")
                          }
                          style={({ pressed }) =>
                            pressed && styles.transcribePressed
                          }
                        >
                          <Text style={styles.transcribeAction}>
                            {!canTranscribe
                              ? "Choose transcription provider"
                              : transcription === "failed"
                                ? "Transcription failed — tap to retry"
                                : "Tap to transcribe"}
                          </Text>
                        </Pressable>
                      ))}
                  </>
                )
              }
            />
          )}
          <View style={[styles.editor, !showMemos && styles.hidden]}>
            {localNoteAttachments.length > 0 && (
              <View style={styles.attachments}>
                <Text style={styles.attachmentLabel}>Files</Text>
                {localNoteAttachments.map(({ attachment, file }) => (
                  <NoteAttachmentCard
                    key={attachment.attachmentId}
                    availableLocally={file !== null}
                    cloudAvailable={Boolean(
                      attachment.cloudObjectKey &&
                      auth.billing.isPro &&
                      auth.session?.access_token &&
                      env.supabaseUrl,
                    )}
                    errorMessage={
                      attachmentActionError?.attachmentId ===
                      attachment.attachmentId
                        ? attachmentActionError.message
                        : null
                    }
                    filename={attachment.filename}
                    loading={attachmentActionId === attachment.attachmentId}
                    onDownload={() => void handleDownloadAttachment(attachment)}
                    onShare={() => {
                      if (file)
                        void handleShareAttachment(attachment, file.uri);
                    }}
                    sizeBytes={attachment.sizeBytes}
                  />
                ))}
              </View>
            )}
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
            <NoteConflictBanner
              sessionId={id}
              onBeforeRestore={flush}
              onRestored={handleRestored}
            />
            <BodyEditor
              key={`${data.id}:${restored?.bodyToken ?? 0}`}
              accessoryId={`note-editor-controls-${data.id}`}
              defaultBodyFormat={restored?.bodyFormat ?? data.bodyFormat}
              defaultValue={restored?.bodyText ?? data.noteText}
              editable={data.plainEditable}
              onAttach={handleAttachFile}
              onChangeText={(body, bodyFormat) => onEdit({ body, bodyFormat })}
              onCommit={flush}
              onFocusChange={setEditorFocused}
            />
          </View>
        </View>
      )}

      {showEmptyNoteCta && (
        <StartListeningButton onPress={handleListeningAction} />
      )}

      <NoteActionsSheet
        hasRecordingHistory={hasRecordingHistory}
        listening={active}
        onClose={() => setActionsOpen(false)}
        onDelete={() => void handleDelete()}
        onExport={() => void handleExport()}
        onSelectFolder={() => setFoldersOpen(true)}
        onImportRecording={() => void handleImportRecording()}
        onToggleListening={handleListeningAction}
        onVersionHistory={() => setVersionsOpen(true)}
        visible={actionsOpen}
      />

      <FolderPickerSheet
        sessionId={id}
        visible={foldersOpen}
        onClose={() => setFoldersOpen(false)}
      />

      <VersionHistorySheet
        sessionId={id}
        visible={versionsOpen}
        onBeforeRestore={flush}
        onClose={() => setVersionsOpen(false)}
        onRestored={handleRestored}
      />

      {active && !keyboardVisible && (
        <ListeningSheet
          phase={recorder.phase}
          failure={recorder.failure}
          amplitude={recorder.amplitude}
          durationMs={recorder.durationMs}
          onStop={() => void handleStop()}
          onRetry={() => void handleRetryRecording()}
          onOpenSettings={() => void handleOpenSettings()}
        />
      )}
    </SafeAreaView>
  );
}

const useStyles = createStyleHook((Colors) => ({
  safeArea: {
    flex: 1,
    backgroundColor: Colors.background,
  },
  header: {
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "space-between",
    paddingHorizontal: Spacing.md,
    paddingVertical: Spacing.sm,
  },
  editor: {
    flex: 1,
  },
  title: {
    flex: 1,
    minWidth: 0,
    paddingHorizontal: Spacing.sm,
    ...Typography.bodyStrong,
    textAlign: "center",
    color: Colors.ink,
  },
  tabs: { marginHorizontal: Spacing.md, marginVertical: Spacing.md },
  hidden: { display: "none" },
  summary: {
    paddingHorizontal: Spacing.md,
    paddingBottom: Spacing.lg,
    gap: Spacing.md,
  },
  summaryTitle: { ...Typography.section, color: Colors.ink },
  summaryScroll: { flex: 1 },
  summaryText: { ...Typography.body, color: Colors.ink },
  summaryError: {
    ...Typography.caption,
    color: Colors.accent,
    marginTop: Spacing.sm,
  },
  transcribeStatus: {
    marginHorizontal: Spacing.md,
    marginTop: Spacing.xs,
    ...Typography.caption,
    color: Colors.muted,
  },
  transcribeAction: {
    marginHorizontal: Spacing.md,
    marginTop: Spacing.xs,
    ...Typography.captionStrong,
    color: Colors.ink,
  },
  transcribePressed: {
    opacity: 0.6,
  },
  attachments: {
    gap: Spacing.sm,
    marginHorizontal: Spacing.md,
    marginTop: Spacing.md,
  },
  attachmentLabel: {
    ...Typography.captionStrong,
    color: Colors.muted,
  },
  readOnlyChip: {
    flexDirection: "row",
    alignItems: "center",
    gap: Spacing.xs,
    marginHorizontal: Spacing.md,
    marginTop: Spacing.sm,
  },
  readOnlyLabel: {
    ...Typography.caption,
    color: Colors.muted,
  },
  body: {
    flex: 1,
    paddingHorizontal: Spacing.md,
    paddingTop: Spacing.md,
    ...Typography.body,
    color: Colors.ink,
  },
  androidAccessory: {
    backgroundColor: Colors.background,
  },
}));
