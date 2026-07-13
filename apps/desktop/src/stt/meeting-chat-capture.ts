import { commands as detectCommands } from "@hypr/plugin-detect";
import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";

import { showTransientToast } from "~/sidebar/toast/transient";
import { persistMeetingChatRecords } from "~/stt/meeting-chat-records";

const MEETING_CHAT_CAPTURE_INTERVAL_MS = 5_000;
const SUPPORTED_MEETING_CHAT_BUNDLE_IDS = new Set([
  "us.zoom.xos",
  "com.tinyspeck.slackmacgap",
]);

export function startMeetingChatCapture({
  sessionId,
  isEnabled,
  excludedTexts = [],
}: {
  sessionId: string;
  isEnabled: () => boolean;
  excludedTexts?: string[];
}) {
  const excludedMessages = new Set(excludedTexts.map(normalizeMessageText));
  const seenSignatures = new Set<string>();
  let baselineBundleId: string | null = null;
  let stopped = false;
  let inFlight = false;
  let lastWarning = "";

  const capture = async () => {
    if (stopped || inFlight) {
      return;
    }
    if (!isEnabled()) {
      baselineBundleId = null;
      return;
    }

    inFlight = true;
    try {
      const applications = await detectCommands.listMicUsingApplications();
      if (stopped || !isEnabled()) {
        baselineBundleId = null;
        return;
      }
      if (applications.status === "error") {
        baselineBundleId = null;
        console.warn(
          "[listener] failed to identify active meeting app",
          applications.error,
        );
        return;
      }

      const bundleIds = [
        ...new Set(
          applications.data
            .map((app) => app.id)
            .filter((bundleId) =>
              SUPPORTED_MEETING_CHAT_BUNDLE_IDS.has(bundleId),
            ),
        ),
      ];
      if (bundleIds.length !== 1) {
        baselineBundleId = null;
        return;
      }

      const bundleId = bundleIds[0];
      const result = await detectCommands.captureMeetingChatMessages([
        bundleId,
      ]);
      if (stopped || !isEnabled()) {
        baselineBundleId = null;
        return;
      }
      if (result.status === "error") {
        baselineBundleId = null;
        console.warn("[listener] failed to capture meeting chat", result.error);
        return;
      }

      showCaptureWarning(result.data.warnings, lastWarning);
      lastWarning = result.data.warnings.join("\n");

      if (result.data.app?.id !== bundleId) {
        baselineBundleId = null;
        return;
      }

      const messages = result.data.messages.filter(
        (message) => !excludedMessages.has(normalizeMessageText(message.text)),
      );
      if (baselineBundleId !== bundleId) {
        baselineBundleId = bundleId;
        for (const message of messages) {
          seenSignatures.add(getCapturedMeetingChatSignature(message));
        }
        return;
      }

      const pendingSignatures = new Set<string>();
      const entries = messages.flatMap((message) => {
        const sourceSignature = getCapturedMeetingChatSignature(message);
        if (
          seenSignatures.has(sourceSignature) ||
          pendingSignatures.has(sourceSignature)
        ) {
          return [];
        }

        pendingSignatures.add(sourceSignature);
        return [{ message, sourceSignature }];
      });
      if (entries.length === 0) {
        return;
      }

      let persistedSignatures: string[];
      try {
        persistedSignatures = await persistMeetingChatRecords({
          sessionId,
          entries,
        });
      } catch (error) {
        console.warn("[listener] failed to persist meeting chat", error);
        return;
      }
      if (stopped || !isEnabled()) {
        return;
      }
      for (const signature of persistedSignatures) {
        seenSignatures.add(signature);
      }
    } catch (error) {
      baselineBundleId = null;
      console.warn("[listener] failed to capture meeting chat", error);
    } finally {
      inFlight = false;
    }
  };

  void capture();
  const interval = setInterval(() => {
    void capture();
  }, MEETING_CHAT_CAPTURE_INTERVAL_MS);

  return () => {
    stopped = true;
    clearInterval(interval);
  };
}

function showCaptureWarning(warnings: string[], previousWarning: string) {
  const warning = warnings.join("\n");
  if (warning && warning !== previousWarning) {
    console.warn("[listener] meeting chat capture warning", warning);
  }
  if (
    warning.includes("accessibility permission") &&
    warning !== previousWarning
  ) {
    showTransientToast(
      {
        id: "meeting-chat-capture-warning",
        description:
          "Meeting chat capture needs Accessibility permission in Settings",
        variant: "warning",
      },
      { durationMs: 6_000 },
    );
  }
}

function getCapturedMeetingChatSignature(message: MeetingCapturedChatMessage) {
  return message.id
    ? [message.platform, message.surface, message.id].join("\n")
    : [
        message.platform,
        message.surface,
        message.sender ?? "",
        message.timestamp ?? "",
        message.text,
      ].join("\n");
}

function normalizeMessageText(text: string) {
  return text.replace(/\s+/g, " ").trim();
}
