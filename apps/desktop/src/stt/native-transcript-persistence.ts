import {
  commands as transcriptionCommands,
  events as transcriptionEvents,
  type LiveTranscriptPersistence,
} from "@anlg/plugin-transcription";

const FLUSH_TIMEOUT_MS = 20_000;

type CreateNativeTranscriptPersistenceOptions = {
  sessionId: string;
  transcriptId: string;
  onPersisted: (status: LiveTranscriptPersistence) => void;
  onError: (error: unknown) => void;
  afterFlush: () => Promise<void>;
};

export function createNativeTranscriptPersistence({
  sessionId,
  transcriptId,
  onPersisted,
  onError,
  afterFlush,
}: CreateNativeTranscriptPersistenceOptions) {
  let pendingFailure = false;
  let disposed = false;
  let unlisten: (() => void) | undefined;

  const applyStatus = (status: LiveTranscriptPersistence) => {
    if (
      disposed ||
      status.session_id !== sessionId ||
      status.transcript_id !== transcriptId
    ) {
      return;
    }

    if (status.error) {
      pendingFailure = true;
      onError(new Error(status.error));
      return;
    }

    pendingFailure = false;
    onPersisted(status);
  };

  void transcriptionEvents.liveTranscriptPersistenceEvent
    .listen(({ payload }) => applyStatus(payload.status))
    .then((stopListening) => {
      if (disposed) {
        stopListening();
      } else {
        unlisten = stopListening;
      }
    })
    .catch((error: unknown) => {
      if (!disposed) onError(error);
    });

  const flush = async () => {
    if (disposed) return;

    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    let timedOut = false;
    const operation = (async () => {
      const result = await transcriptionCommands.flushLiveTranscript(sessionId);
      if (timedOut || disposed) return;

      if (result.status === "error") {
        throw new Error(result.error);
      }

      const status = result.data;
      if (status) applyStatus(status);
      if (!status?.error) await afterFlush();
    })();
    const timeout = new Promise<never>((_resolve, reject) => {
      timeoutId = setTimeout(() => {
        timedOut = true;
        reject(new Error("transcript persistence flush timed out"));
      }, FLUSH_TIMEOUT_MS);
    });

    try {
      await Promise.race([operation, timeout]);
    } catch (error) {
      pendingFailure = true;
      onError(error);
    } finally {
      if (timeoutId) clearTimeout(timeoutId);
    }
  };

  return {
    flush,
    hasPendingFailure: () => pendingFailure,
    dispose: () => {
      if (disposed) return;
      disposed = true;
      unlisten?.();
      unlisten = undefined;
      void transcriptionCommands
        .releaseLiveTranscript(sessionId, transcriptId)
        .catch((error: unknown) => {
          console.error(
            "[listener] failed to release live transcript journal",
            error,
          );
        });
    },
  };
}
