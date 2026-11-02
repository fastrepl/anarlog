import { useQuery } from "@tanstack/react-query";
import { useRouter } from "expo-router";
import { Alert, ScrollView, Text, View } from "react-native";

import { useAuth } from "@/auth/context";
import { MarkdownView } from "@/components/markdown-view";
import { RecordingInterruptionNotice } from "@/components/recording-interruption-notice";
import { RemoteAudioCard } from "@/components/remote-audio-card";
import { Button } from "@/components/ui/button";
import { Spacing, Typography } from "@/constants/theme";
import type { SessionDetail } from "@/data/session-detail";
import {
  notifySummarySkipped,
  SummarySkippedError,
  summarizeSession,
  summaryRecoveryOptions,
  useSessionSummaryState,
} from "@/data/summarize";
import { transcribeSession, useTranscriptionState } from "@/data/transcribe";
import { readProviderConfig } from "@/settings/providers";
import { providersFor } from "@/settings/providers-model";
import { createStyleHook } from "@/settings/theme-provider";
import { useProviderAccess } from "@/settings/use-provider-access";

import type { useNoteAudio } from "./use-note-audio";

export function NoteSummary({
  sessionId,
  active,
  visible,
  summary,
  hasTranscript,
  audio,
  onBeforeGenerate,
  onOpenEvidence,
}: {
  sessionId: string;
  active: boolean;
  visible: boolean;
  summary: SessionDetail["summary"];
  hasTranscript: boolean;
  audio: ReturnType<typeof useNoteAudio>;
  onBeforeGenerate: () => Promise<void> | void;
  onOpenEvidence?: (segmentId: string) => void;
}) {
  const styles = useStyles();
  const router = useRouter();
  const auth = useAuth();
  const account = auth.session?.user.id ?? null;
  const canTranscribe = useProviderAccess("stt");
  const canSummarize = useProviderAccess("llm");
  const summaryProvider = useQuery({
    queryKey: ["provider", account, "llm"],
    queryFn: () => readProviderConfig(account, "llm"),
  });
  const summaryState = useSessionSummaryState(sessionId);
  const transcription = useTranscriptionState(sessionId);
  useQuery(
    summaryRecoveryOptions(
      sessionId,
      visible &&
        !active &&
        hasTranscript &&
        canSummarize &&
        audio.data?.transcriptStatus === "complete" &&
        transcription !== "running",
    ),
  );
  const summaryPending = summaryState?.status === "pending";
  const summaryError = summaryState?.error;
  const summarySkipped = summaryError instanceof SummarySkippedError;
  const usesCloudflareWorker =
    summaryProvider.data?.provider === "cloudflare_worker";
  const summaryProviderName = summaryProvider.data
    ? (providersFor("llm").find(
        (provider) => provider.id === summaryProvider.data?.provider,
      )?.name ?? summaryProvider.data.provider)
    : null;
  const needsTranscription =
    Boolean(audio.data) && audio.data?.transcriptStatus !== "complete";
  const generate = () =>
    void summarizeSession(sessionId, {
      beforeGenerate: onBeforeGenerate,
    }).catch((error) => notifySummarySkipped(sessionId, error));

  if (!visible) return null;
  return (
    <ScrollView
      style={styles.summaryScroll}
      contentContainerStyle={styles.summary}
    >
      {summary && (
        <View>
          {summary.outdated && (
            <Text accessibilityRole="alert" style={styles.summaryOutdated}>
              Transcript edited since this summary was generated. Regenerate it
              to include the latest changes.
            </Text>
          )}
          {summary.title !== "Summary" && (
            <Text style={styles.summaryTitle}>{summary.title}</Text>
          )}
          <MarkdownView
            markdown={summary.text}
            onLinkPress={(href) => {
              const match = /^anarlog-evidence:\/\/(.+)$/u.exec(href);
              if (!match) return;
              try {
                onOpenEvidence?.(decodeURIComponent(match[1]));
              } catch {
                /* malformed link */
              }
            }}
          />
        </View>
      )}
      {audio.data?.captureInterruptedAtMs !== null &&
        audio.data?.captureInterruptedAtMs !== undefined && (
          <RecordingInterruptionNotice />
        )}
      {!summary && (
        <Text style={styles.summaryText}>
          {active
            ? "After the meeting, review the transcript and tap Generate summary. Anarlog will ask before sending meeting text to your selected provider."
            : summaryPending
              ? "Generating summary…"
              : transcription === "running"
                ? "Finishing transcription. When it completes, review the transcript and tap Generate summary to continue."
                : needsTranscription
                  ? "Your recording is saved. Finish transcription to get your summary."
                  : "Your meeting summary will appear here. Your memos are in the Memos tab."}
        </Text>
      )}
      {!active && needsTranscription && transcription !== "running" && (
        <>
          {!audio.availableLocally && <RemoteAudioCard {...audio.restore} />}
          {audio.availableLocally && (
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
                  ? void transcribeSession(sessionId)
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
        (!canSummarize || summaryError || summary || hasTranscript) && (
          <Button
            label={
              !canSummarize
                ? "Choose summary provider"
                : summaryError && !summarySkipped
                  ? "Retry summary"
                  : summary
                    ? "Regenerate summary"
                    : "Generate summary"
            }
            loading={summaryPending}
            disabled={transcription === "running" || !summaryProvider.data}
            variant="ghost"
            size="small"
            onPress={() => {
              if (!canSummarize) {
                router.push("/settings/summary-provider");
                return;
              }
              const config = summaryProvider.data;
              if (!config || !summaryProviderName) return;
              const destination = config.baseUrl.trim() || summaryProviderName;
              const authenticationNotice =
                usesCloudflareWorker || config.provider === "anarlog"
                  ? " Your Anarlog session token will also be sent there to verify your account."
                  : "";
              Alert.alert(
                `Send meeting text to ${summaryProviderName}?`,
                `Your notes and transcript text will be sent to ${destination} for this request. Audio is not sent.${authenticationNotice} Continue only if you trust this destination.`,
                [
                  { text: "Cancel", style: "cancel" },
                  { text: "Send and generate", onPress: generate },
                ],
              );
            }}
          />
        )}
    </ScrollView>
  );
}

const useStyles = createStyleHook((Colors) => ({
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
  summaryOutdated: {
    ...Typography.caption,
    color: Colors.destructive,
    marginBottom: Spacing.sm,
  },
}));
