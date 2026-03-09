import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useRef } from "react";

import { sttListenBatch, sttStatus } from "@hypr/api-client";
import type {
  ListenCallbackResponse,
  SttStatusResponse,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import {
  buildSegments,
  ChannelProfile,
  type RuntimeSpeakerHint,
  type Segment,
  type WordLike,
} from "@hypr/transcript";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";
import { useAudioUppy } from "@/hooks/use-audio-uppy";
import { useSummaryStream } from "@/hooks/use-summary-stream";
import type { SessionCell, Store } from "@/store/tinybase";

function createAuthClient(accessToken: string) {
  return createClient({
    baseUrl: env.VITE_API_URL,
    headers: { Authorization: `Bearer ${accessToken}` },
  });
}

type RawWord = {
  word: string;
  start: number;
  end: number;
  confidence: number;
  speaker?: number | null;
  punctuated_word?: string | null;
};

type RawResults = {
  channels?: Array<{
    alternatives?: Array<{
      transcript?: string;
      words?: RawWord[];
    }>;
  }>;
};

function extractTranscriptData(response: SttStatusResponse): {
  text: string | null;
  segments: Segment[] | null;
} {
  if (response.status !== "done" || !response.rawResult) {
    return { text: null, segments: null };
  }

  if (typeof response.rawResult.text === "string") {
    return { text: response.rawResult.text, segments: null };
  }

  const results = response.rawResult.results as RawResults | undefined;
  const alternative = results?.channels?.[0]?.alternatives?.[0];
  const text =
    typeof alternative?.transcript === "string" ? alternative.transcript : null;

  const rawWords = alternative?.words;
  if (!rawWords || rawWords.length === 0) {
    return { text, segments: null };
  }

  const words: (WordLike & { id: string })[] = [];
  const speakerHints: RuntimeSpeakerHint[] = [];

  for (let i = 0; i < rawWords.length; i++) {
    const w = rawWords[i];
    words.push({
      id: `w${i}`,
      text: (w.punctuated_word ?? w.word) + " ",
      start_ms: Math.round(w.start * 1000),
      end_ms: Math.round(w.end * 1000),
      channel: ChannelProfile.MixedCapture,
    });

    if (w.speaker != null) {
      speakerHints.push({
        wordIndex: i,
        data: { type: "provider_speaker_index", speaker_index: w.speaker },
      });
    }
  }

  const segments = buildSegments(words, [], speakerHints);
  return { text, segments };
}

export function useTranscriptionPipeline(
  sessionId: string,
  store: Store | undefined,
) {
  const summaryTriggeredRef = useRef(false);

  const pipelineId = store?.getCell("sessions", sessionId, "pipeline_id") as
    | string
    | undefined;

  const setCell = useCallback(
    (cell: SessionCell, value: string | number | boolean) => {
      store?.setCell("sessions", sessionId, cell, value);
    },
    [store, sessionId],
  );

  const {
    summary,
    isStreaming: isSummarizing,
    error: summaryError,
    generate: generateSummary,
  } = useSummaryStream();

  const {
    addFile: uppyAddFile,
    status: uppyStatus,
    progress: uppyProgress,
    fileId: uppyFileId,
    error: uppyError,
  } = useAudioUppy();

  useEffect(() => {
    setCell("upload_progress", uppyProgress);
  }, [uppyProgress, setCell]);

  useEffect(() => {
    setCell("summary", summary);
  }, [summary, setCell]);

  useEffect(() => {
    setCell("is_summarizing", isSummarizing);
  }, [isSummarizing, setCell]);

  useEffect(() => {
    setCell("summary_error", summaryError ?? "");
  }, [summaryError, setCell]);

  const startPipelineMutation = useMutation({
    mutationFn: async (fileId: string) => {
      const token = await getAccessToken();
      const client = createAuthClient(token);
      const { data, error } = await sttListenBatch({
        client,
        body: { url: fileId },
        query: { callback: "true", provider: "deepgram" },
      });
      if (error || !data) {
        const detail =
          typeof error === "object" && error !== null
            ? ((error as Record<string, unknown>).detail ??
              (error as Record<string, unknown>).error ??
              JSON.stringify(error))
            : String(error ?? "unknown error");
        throw new Error(`Failed to start transcription: ${detail}`);
      }
      return (data as unknown as ListenCallbackResponse).request_id;
    },
    onSuccess: (newPipelineId) => {
      setCell("pipeline_id", newPipelineId);
      const url = new URL(window.location.href);
      url.searchParams.set("id", newPipelineId);
      window.history.replaceState({}, "", url.toString());
    },
  });

  const pipelineStatusQuery = useQuery({
    queryKey: ["audioPipelineStatus", pipelineId],
    queryFn: async (): Promise<SttStatusResponse> => {
      if (!pipelineId) {
        throw new Error("Missing pipelineId");
      }
      const token = await getAccessToken();
      const client = createAuthClient(token);
      const { data, error } = await sttStatus({
        client,
        path: { pipeline_id: pipelineId },
      });
      if (error) {
        throw new Error("Failed to get status");
      }
      return data!;
    },
    enabled: !!pipelineId,
    refetchInterval: (query) => {
      const s = query.state.data?.status;
      return s === "done" || s === "error" ? false : 2000;
    },
  });

  useEffect(() => {
    const data = pipelineStatusQuery.data;
    if (data) {
      const { text, segments } = extractTranscriptData(data);
      if (text) {
        setCell("transcript", text);
      }
      if (segments) {
        setCell("transcript_segments", JSON.stringify(segments));
      }
    }
  }, [pipelineStatusQuery.data, setCell]);

  const transcript = store?.getCell("sessions", sessionId, "transcript") as
    | string
    | undefined;

  const transcriptSegmentsRaw = store?.getCell(
    "sessions",
    sessionId,
    "transcript_segments",
  ) as string | undefined;

  const transcriptSegments: Segment[] | null = (() => {
    if (!transcriptSegmentsRaw) return null;
    try {
      return JSON.parse(transcriptSegmentsRaw) as Segment[];
    } catch {
      return null;
    }
  })();

  useEffect(() => {
    if (transcriptSegments && !summaryTriggeredRef.current) {
      summaryTriggeredRef.current = true;
      generateSummary(transcriptSegments);
    }
  }, [transcriptSegments, generateSummary]);

  useEffect(() => {
    if (uppyStatus === "done" && uppyFileId && !pipelineId) {
      startPipelineMutation.mutate(uppyFileId);
    }
  }, [uppyStatus, uppyFileId, pipelineId]);

  const pipelineStatus = pipelineStatusQuery.data?.status;

  useEffect(() => {
    const status = (() => {
      if (
        pipelineStatus === "error" ||
        startPipelineMutation.isError ||
        pipelineStatusQuery.isError
      ) {
        return "error";
      }
      if (pipelineStatus === "done" || transcript) return "done";
      if (
        pipelineStatus === "processing" ||
        pipelineId ||
        startPipelineMutation.isPending
      ) {
        return "transcribing";
      }
      if (uppyStatus === "uploading") return "uploading";
      if (uppyStatus === "error") return "error";
      if (uppyStatus === "done" && uppyFileId) return "uploaded";
      return "idle";
    })();
    setCell("pipeline_status", status);

    const errorMessage =
      uppyError ??
      (startPipelineMutation.error instanceof Error
        ? startPipelineMutation.error.message
        : null) ??
      (pipelineStatusQuery.isError && pipelineStatusQuery.error instanceof Error
        ? pipelineStatusQuery.error.message
        : null) ??
      (pipelineStatus === "error"
        ? (pipelineStatusQuery.data?.error ?? null)
        : null) ??
      "";
    setCell("error_message", errorMessage);
  }, [
    pipelineStatus,
    transcript,
    pipelineId,
    startPipelineMutation.isPending,
    startPipelineMutation.isError,
    startPipelineMutation.error,
    uppyStatus,
    uppyFileId,
    uppyError,
    pipelineStatusQuery.isError,
    pipelineStatusQuery.error,
    pipelineStatusQuery.data?.error,
    setCell,
  ]);

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const selectedFile = e.target.files?.[0];
      if (!selectedFile) return;
      setCell("file_name", selectedFile.name);
      setCell("pipeline_id", "");
      setCell("transcript", "");
      setCell("transcript_segments", "");
      setCell("summary", "");
      setCell("summary_error", "");
      summaryTriggeredRef.current = false;
      startPipelineMutation.reset();
      uppyAddFile(selectedFile);
    },
    [setCell, startPipelineMutation, uppyAddFile],
  );

  const handleRegenerate = useCallback(() => {
    if (transcriptSegments) {
      summaryTriggeredRef.current = true;
      generateSummary(transcriptSegments);
    }
  }, [transcriptSegments, generateSummary]);

  return { handleFileSelect, handleRegenerate };
}
