import * as Sharing from "expo-sharing";

import type { HandoffStatus } from "@/data/handoff-status";
import { captureAnalytics } from "@/lib/analytics";
import { captureOperationalError } from "@/lib/error-reporting";

const CONTENT_TYPES: Record<string, string> = {
  m4a: "audio/mp4",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  ogg: "audio/ogg",
  caf: "audio/x-caf",
};

export async function handoffRecording(
  uri: string,
  filename: string,
): Promise<HandoffStatus> {
  try {
    if (!(await Sharing.isAvailableAsync())) {
      captureAnalytics("handoff_failed", {
        transport: "system_share",
        failure_stage: "unavailable",
      });
      return "unavailable";
    }

    const extension = filename.split(".").pop()?.toLowerCase() ?? "";
    captureAnalytics("handoff_started", {
      transport: "system_share",
      content_type: "recording",
    });
    await Sharing.shareAsync(uri, {
      dialogTitle: "Send recording to Anarlog Desktop",
      mimeType: CONTENT_TYPES[extension] ?? "application/octet-stream",
      UTI: "public.audio",
    });
    captureAnalytics("handoff_completed", {
      transport: "system_share",
      confirmation_level: "share_sheet_closed",
      desktop_import_confirmed: false,
    });
    return "shared_unconfirmed";
  } catch (error) {
    captureOperationalError(error, {
      operation: "recording_handoff",
      tags: { transport: "system_share" },
    });
    captureAnalytics("handoff_failed", {
      transport: "system_share",
      failure_stage: "share_sheet",
    });
    return "failed";
  }
}
