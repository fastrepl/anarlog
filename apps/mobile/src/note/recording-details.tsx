import { useRouter } from "expo-router";
import { Pressable, Text, View } from "react-native";

import { AudioChip } from "@/components/audio-chip";
import { RecordingSyncCard } from "@/components/recording-sync-card";
import { RemoteAudioCard } from "@/components/remote-audio-card";
import { Spacing, Typography } from "@/constants/theme";
import { transcribeSession, useTranscriptionState } from "@/data/transcribe";
import { createStyleHook } from "@/settings/theme-provider";
import { useProviderAccess } from "@/settings/use-provider-access";

import type { useNoteAudio } from "./use-note-audio";

export function RecordingDetails({
  sessionId,
  audio,
  hasTranscript,
}: {
  sessionId: string;
  audio: ReturnType<typeof useNoteAudio>;
  hasTranscript: boolean | undefined;
}) {
  const styles = useStyles();
  const router = useRouter();
  const canTranscribe = useProviderAccess("stt");
  const transcription = useTranscriptionState(sessionId);
  return (
    <>
      {audio.data && audio.availableLocally && audio.file && (
        <View key={`${audio.data.filename}:${audio.data.createdAt}`}>
          <AudioChip
            uri={audio.file.uri}
            filename={audio.data.filename}
            sizeBytes={audio.data.sizeBytes}
          />
          <RecordingSyncCard audio={audio.data} />
        </View>
      )}
      {audio.data && !audio.availableLocally && (
        <RemoteAudioCard {...audio.restore} />
      )}
      {audio.data &&
        audio.availableLocally &&
        audio.data.transcriptStatus !== "complete" &&
        hasTranscript === false &&
        (transcription === "running" ? (
          <Text style={styles.transcribeStatus}>Transcribing…</Text>
        ) : (
          <Pressable
            hitSlop={4}
            onPress={() =>
              canTranscribe
                ? void transcribeSession(sessionId)
                : router.push("/settings/transcription-provider")
            }
            style={({ pressed }) => pressed && styles.transcribePressed}
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
  );
}

const useStyles = createStyleHook((Colors) => ({
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
  transcribePressed: { opacity: 0.6 },
}));
