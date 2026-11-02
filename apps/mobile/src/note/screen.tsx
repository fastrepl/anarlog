import SegmentedControl from "@expo/ui/community/segmented-control";
import { Ionicons } from "@expo/vector-icons";
import * as Linking from "expo-linking";
import { useRouter } from "expo-router";
import { useRef, useState } from "react";
import { Alert, Keyboard, Text, TextInput, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

import { useSessionRecorder } from "@/audio/use-session-recorder";
import { FolderPickerSheet } from "@/components/folder-picker-sheet";
import { ListeningSheet } from "@/components/listening-sheet";
import { NoteActionsSheet } from "@/components/note-actions-sheet";
import { MeetingGlossarySheet } from "@/components/meeting-glossary-sheet";
import { NoteConflictBanner } from "@/components/note-conflict-banner";
import { SessionTranscript } from "@/components/session-transcript";
import { StartListeningButton } from "@/components/start-listening-button";
import { IconButton } from "@/components/ui/icon-button";
import { VersionHistorySheet } from "@/components/version-history-sheet";
import { Spacing, Typography } from "@/constants/theme";
import { exportMeetingPackage } from "@/data/meeting-package";
import { useNoteAttachments } from "@/data/note-attachment-catalog";
import { pickAndCatalogNoteAttachment } from "@/data/note-attachments";
import { deleteSession, useSessionDetail } from "@/data/session";
import { sessionView, type SessionViewSelection } from "@/data/session-view";
import {
  loadSessionTranscripts,
  useSessionHasTranscript,
} from "@/data/transcripts";
import { captureAnalytics } from "@/lib/analytics";
import { confirmDestructive } from "@/lib/confirm";
import { captureOperationalError } from "@/lib/error-reporting";
import { useKeyboardVisible } from "@/lib/use-keyboard-visible";
import { useMountEffect } from "@/lib/use-mount-effect";
import { createStyleHook, useColors } from "@/settings/theme-provider";

import { BodyEditor } from "./body-editor";
import { NoteFiles } from "./note-files";
import { NoteSummary } from "./note-summary";
import { RecordingDetails } from "./recording-details";
import { useNoteAudio } from "./use-note-audio";
import { useNoteDraft } from "./use-note-draft";

export function NoteScreen({
  id,
  autoListen,
}: {
  id: string;
  autoListen: boolean;
}) {
  const styles = useStyles();
  const Colors = useColors();
  const router = useRouter();
  const { data, isLoading } = useSessionDetail(id);
  const noteAttachments = useNoteAttachments(id);
  const transcriptState = useSessionHasTranscript(id);
  const audio = useNoteAudio(id, transcriptState.data === true);
  const {
    edit: onEdit,
    flush,
    remove,
    restored,
    bodyDraft,
    onRestored: handleRestored,
  } = useNoteDraft(id, data);
  const [selection, setSelection] = useState<SessionViewSelection | null>(null);
  const [evidenceSeek, setEvidenceSeek] = useState<{
    id: number;
    segmentId: string;
    seconds: number;
  } | null>(null);
  const evidenceRequestId = useRef(0);
  const [listening, setListening] = useState(autoListen);
  const [editorFocused, setEditorFocused] = useState(false);
  const [sheet, setSheet] = useState<
    "actions" | "folders" | "glossary" | "versions" | null
  >(null);
  const closeSheet = (name: typeof sheet) =>
    setSheet((current) => (current === name ? null : current));
  const keyboardVisible = useKeyboardVisible();
  const recorder = useSessionRecorder(id, listening);
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
  const openEvidence = async (segmentId: string) => {
    const segments = await loadSessionTranscripts(id);
    const segment = segments.find((item) => item.id === segmentId);
    if (!segment) {
      Alert.alert(
        "Evidence unavailable",
        "This transcript segment is no longer available.",
      );
      return;
    }
    setSelection({ sessionId: id, active, view: { type: "transcript" } });
    const captureStart = audio.data?.captureStartedAtMs;
    const baseTime =
      captureStart ?? Math.min(...segments.map((item) => item.startMs));
    setEvidenceSeek({
      id: ++evidenceRequestId.current,
      segmentId,
      seconds: Number.isFinite(baseTime)
        ? Math.max(0, (segment.startMs - baseTime) / 1000)
        : 0,
    });
  };
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

  useMountEffect(() => {
    captureAnalytics("note_opened", { entry_point: "mobile_note" });
  });

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

  const handleDelete = async () => {
    const confirmed = await confirmDestructive(
      `Delete "${data?.title || "Untitled"}"?`,
      "Delete",
    );
    if (!confirmed) return;
    await recorder.stop();
    try {
      await remove(() => deleteSession(id));
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
    if (!data) return;
    try {
      await flush(true);
    } catch (error) {
      captureOperationalError(error, {
        operation: "session_export",
        tags: { entry_point: "mobile_note" },
      });
      Alert.alert(
        "Couldn’t export meeting",
        error instanceof Error ? error.message : "Try again.",
      );
      return;
    }
    const canIncludeAudio = audio.availableLocally && Boolean(audio.file);
    const runExport = (includeAudio: boolean) => {
      void exportMeetingPackage(id, includeAudio).catch((error: unknown) => {
        captureOperationalError(error, {
          operation: "session_export",
          tags: { entry_point: "mobile_note" },
        });
        Alert.alert(
          "Couldn’t export meeting",
          error instanceof Error ? error.message : "Try again.",
        );
      });
    };
    Alert.alert(
      "Export meeting",
      canIncludeAudio
        ? "Choose whether to include the recording. Audio stays on this device unless you share the exported package."
        : "The recording is not available on this device. The package will contain meeting text only.",
      [
        { text: "Without audio", onPress: () => runExport(false) },
        ...(canIncludeAudio
          ? [{ text: "Include audio", onPress: () => runExport(true) }]
          : []),
        { text: "Cancel", style: "cancel" },
      ],
    );
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
    setSheet("actions");
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
          <NoteSummary
            sessionId={id}
            active={active}
            visible={current.type === "enhanced"}
            summary={data.summary}
            hasTranscript={transcriptState.data === true}
            audio={audio}
            onBeforeGenerate={() => flush(true)}
            onOpenEvidence={(segmentId) => void openEvidence(segmentId)}
          />
          {current.type === "transcript" && (
            <SessionTranscript
              sessionId={id}
              focusSegmentId={evidenceSeek?.segmentId}
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
                  <RecordingDetails
                    sessionId={id}
                    audio={audio}
                    hasTranscript={transcriptState.data}
                    seekRequest={
                      evidenceSeek
                        ? { id: evidenceSeek.id, seconds: evidenceSeek.seconds }
                        : null
                    }
                  />
                )
              }
            />
          )}
          <View style={[styles.editor, !showMemos && styles.hidden]}>
            <NoteFiles sessionId={id} attachments={noteAttachments} />
            {data.document && !data.plainEditable && (
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
            {!data.document ? (
              <Text style={styles.readOnlyLabel}>
                Waiting for note content to sync…
              </Text>
            ) : (
              <BodyEditor
                key={`${data.id}:${restored?.bodyToken ?? 0}`}
                accessoryId={`note-editor-controls-${data.id}`}
                bodyFormat={bodyDraft?.format ?? data.bodyFormat}
                value={bodyDraft?.text ?? data.noteText}
                editable={data.plainEditable}
                onAttach={handleAttachFile}
                onChangeText={(body, bodyFormat) =>
                  onEdit({ body, bodyFormat })
                }
                onCommit={flush}
                onFocusChange={setEditorFocused}
              />
            )}
          </View>
        </View>
      )}

      {showEmptyNoteCta && (
        <StartListeningButton onPress={handleListeningAction} />
      )}

      <NoteActionsSheet
        hasRecordingHistory={hasRecordingHistory}
        listening={active}
        onClose={() => closeSheet("actions")}
        onDelete={() => void handleDelete()}
        onEditGlossary={() => setSheet("glossary")}
        onExport={() => void handleExport()}
        onSelectFolder={() => setSheet("folders")}
        onImportRecording={audio.importRecording}
        onToggleListening={handleListeningAction}
        onVersionHistory={() => setSheet("versions")}
        visible={sheet === "actions"}
      />

      <FolderPickerSheet
        sessionId={id}
        visible={sheet === "folders"}
        onClose={() => closeSheet("folders")}
      />

      <MeetingGlossarySheet
        sessionId={id}
        visible={sheet === "glossary"}
        onClose={() => closeSheet("glossary")}
      />

      <VersionHistorySheet
        sessionId={id}
        visible={sheet === "versions"}
        onBeforeRestore={flush}
        onClose={() => closeSheet("versions")}
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
}));
