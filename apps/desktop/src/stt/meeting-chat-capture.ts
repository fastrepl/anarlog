import { commands as detectCommands } from "@hypr/plugin-detect";
import type { MeetingCapturedChatMessage } from "@hypr/plugin-detect";
import { sonnerToast } from "@hypr/ui/components/ui/toast";

import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { persistMeetingChatRecords } from "~/stt/meeting-chat-records";

const MEETING_CHAT_CAPTURE_INTERVAL_MS = 5_000;

export function startMeetingChatCapture({
  sessionId,
  isEnabled,
  excludedTexts = [],
}: {
  sessionId: string;
  isEnabled?: () => boolean | Promise<boolean>;
  excludedTexts?: string[];
}) {
  const excludedMessages = new Set(excludedTexts.map(normalizeMessageText));
  const seenSignatures = new Set<string>();
  let baselineContext: { bundleId: string; contextId: string } | null = null;
  let stopped = false;
  let inFlight = false;
  let lastWarning = "";
  const captureIsEnabled =
    isEnabled ??
    (async () =>
      resolveConfigValue(
        "capture_meeting_chat",
        await getStoredSettingValues(),
      ));

  const capture = async () => {
    if (stopped || inFlight) {
      return;
    }
    inFlight = true;
    try {
      if (!(await captureIsEnabled())) {
        baselineContext = null;
        return;
      }

      const applications = await detectCommands.listMicUsingApplications();
      if (stopped || !(await captureIsEnabled())) {
        baselineContext = null;
        return;
      }
      if (applications.status === "error") {
        console.warn(
          "[listener] failed to identify active meeting app",
          applications.error,
        );
        return;
      }

      const bundleIds = [
        ...new Set(applications.data.map((app) => app.id).filter(Boolean)),
      ];
      if (bundleIds.length === 0) {
        return;
      }

      const result = await detectCommands.captureMeetingChatMessages(bundleIds);
      if (stopped || !(await captureIsEnabled())) {
        baselineContext = null;
        return;
      }
      if (result.status === "error") {
        console.warn("[listener] failed to capture meeting chat", result.error);
        return;
      }

      showCaptureWarning(result.data.warnings, lastWarning);
      lastWarning = result.data.warnings.join("\n");

      const contextId = result.data.contextId?.trim();
      const bundleId = result.data.app?.id;
      if (!bundleId || !bundleIds.includes(bundleId) || !contextId) {
        return;
      }

      const messages = result.data.messages.filter(
        (message) => !excludedMessages.has(normalizeMessageText(message.text)),
      );
      if (
        !baselineContext ||
        baselineContext.bundleId !== bundleId ||
        baselineContext.contextId !== contextId
      ) {
        baselineContext = { bundleId, contextId };
        for (const message of messages) {
          seenSignatures.add(
            getCapturedMeetingChatSignature(contextId, message),
          );
        }
        return;
      }

      const pendingSignatures = new Set<string>();
      const entries = messages.flatMap((message) => {
        const sourceSignature = getCapturedMeetingChatSignature(
          contextId,
          message,
        );
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
      if (stopped) {
        return;
      }
      if (!(await captureIsEnabled())) {
        baselineContext = null;
        return;
      }
      for (const signature of persistedSignatures) {
        seenSignatures.add(signature);
      }
    } catch (error) {
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
    sonnerToast.warning(
      "Meeting chat capture needs Accessibility permission in Settings",
      {
        id: "meeting-chat-capture-warning",
        duration: 6_000,
      },
    );
  }
}

function getCapturedMeetingChatSignature(
  contextId: string,
  message: MeetingCapturedChatMessage,
) {
  return message.id
    ? [contextId, message.platform, message.surface, message.id].join("\n")
    : [
        contextId,
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
