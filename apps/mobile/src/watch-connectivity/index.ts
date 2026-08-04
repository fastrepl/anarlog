import { Platform } from "react-native";

import { importWatchRecording } from "@/data/import-voice-memo";
import { captureOperationalError } from "@/lib/error-reporting";

import WatchConnectivityModule, {
  type PendingWatchRecording,
} from "../../modules/watch-connectivity";

let initialized = false;
let activeAccountUserId: string | null | undefined;
let activeImportController = new AbortController();
const importsInFlight = new Set<string>();
const importRetryAttempts = new Map<string, number>();
const importRetryTimers = new Map<string, ReturnType<typeof setTimeout>>();
const MAX_IMPORT_RETRIES = 5;

function clearImportRetry(id: string): void {
  const timer = importRetryTimers.get(id);
  if (timer) {
    clearTimeout(timer);
  }
  importRetryTimers.delete(id);
  importRetryAttempts.delete(id);
}

function clearImportRetries(): void {
  for (const id of importRetryTimers.keys()) {
    clearImportRetry(id);
  }
  importRetryAttempts.clear();
}

function scheduleImportRetry(recording: PendingWatchRecording): void {
  if (
    activeAccountUserId !== recording.accountUserId ||
    importRetryTimers.has(recording.id)
  ) {
    return;
  }

  const attempt = (importRetryAttempts.get(recording.id) ?? 0) + 1;
  if (attempt > MAX_IMPORT_RETRIES) {
    clearImportRetry(recording.id);
    return;
  }
  importRetryAttempts.set(recording.id, attempt);

  const timer = setTimeout(
    () => {
      importRetryTimers.delete(recording.id);
      void importRecording(recording);
    },
    Math.min(1_000 * 2 ** (attempt - 1), 30_000),
  );
  importRetryTimers.set(recording.id, timer);
}

async function importRecording(
  recording: PendingWatchRecording,
): Promise<void> {
  if (
    !WatchConnectivityModule ||
    activeAccountUserId !== recording.accountUserId ||
    importsInFlight.has(recording.id) ||
    importRetryTimers.has(recording.id)
  ) {
    return;
  }

  importsInFlight.add(recording.id);
  const importController = activeImportController;
  try {
    await importWatchRecording(recording, importController.signal);
    if (
      importController.signal.aborted ||
      activeAccountUserId !== recording.accountUserId
    ) {
      return;
    }
    WatchConnectivityModule.markRecordingImported(recording.id);
    clearImportRetry(recording.id);
  } catch (error) {
    if (importController.signal.aborted) {
      return;
    }
    captureOperationalError(error, {
      operation: "watch_recording_import",
      context: { recording_id: recording.id },
    });
    scheduleImportRetry(recording);
  } finally {
    importsInFlight.delete(recording.id);
  }
}

export function initializeWatchConnectivity(): void {
  if (initialized || Platform.OS !== "ios" || !WatchConnectivityModule) {
    return;
  }

  initialized = true;
  WatchConnectivityModule.addListener("onRecordingReceived", (recording) => {
    void importRecording(recording);
  });
}

export function updateWatchAccount(
  account: { userId: string; email: string | null } | null,
): void {
  const nextAccountUserId = account?.userId ?? null;
  if (activeAccountUserId !== nextAccountUserId) {
    activeImportController.abort();
    activeImportController = new AbortController();
    clearImportRetries();
  }
  activeAccountUserId = nextAccountUserId;
  if (Platform.OS !== "ios" || !WatchConnectivityModule) {
    return;
  }

  WatchConnectivityModule.updateAccount(
    account?.userId ?? null,
    account?.email ?? null,
  );
  if (account) {
    for (const recording of WatchConnectivityModule.getPendingRecordings()) {
      void importRecording(recording);
    }
  }
}
