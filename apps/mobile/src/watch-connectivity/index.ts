import { Platform } from "react-native";

import { importWatchRecording } from "@/data/import-voice-memo";
import { captureOperationalError } from "@/lib/error-reporting";

import WatchConnectivityModule, {
  type PendingWatchRecording,
} from "../../modules/watch-connectivity";

let initialized = false;
let activeAccountUserId: string | null | undefined;
const importsInFlight = new Set<string>();

async function importRecording(
  recording: PendingWatchRecording,
): Promise<void> {
  if (
    !WatchConnectivityModule ||
    activeAccountUserId !== recording.accountUserId ||
    importsInFlight.has(recording.id)
  ) {
    return;
  }

  importsInFlight.add(recording.id);
  try {
    await importWatchRecording(recording);
    WatchConnectivityModule.markRecordingImported(recording.id);
  } catch (error) {
    captureOperationalError(error, {
      operation: "watch_recording_import",
      context: { recording_id: recording.id },
    });
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
  activeAccountUserId = account?.userId ?? null;
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
