import { commands as detectCommands } from "@anlg/plugin-detect";
import type { MeetingCapturedChatMessage } from "@anlg/plugin-detect";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { getStoredSettingValues } from "~/settings/queries";
import { resolveConfigValue } from "~/shared/config";
import { persistMeetingChatRecords } from "~/stt/meeting-chat-records";

const MEETING_CHAT_CAPTURE_INTERVAL_MS = 5_000;
const MAX_CAPTURED_CHAT_WINDOW = 1_000;

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
  let inFlight: Promise<void> | null = null;
  let pendingPersistence: Promise<void> | null = null;
  let lastWarning = "";
  const captureIsEnabled =
    isEnabled ??
    (async () =>
      resolveConfigValue(
        "capture_meeting_chat",
        await getStoredSettingValues(),
      ));

  const captureOnce = async () => {
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

      const messages = result.data.messages
        .filter(
          (message) =>
            !excludedMessages.has(normalizeMessageText(message.text)),
        )
        .slice(-MAX_CAPTURED_CHAT_WINDOW);
      if (
        !baselineContext ||
        baselineContext.bundleId !== bundleId ||
        baselineContext.contextId !== contextId
      ) {
        baselineContext = { bundleId, contextId };
        seenSignatures.clear();
        for (const message of messages) {
          rememberSignature(
            seenSignatures,
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

      if (stopped) {
        return;
      }

      let persistedSignatures: string[];
      const persistence = persistMeetingChatRecords({
        sessionId,
        entries,
      });
      const settledPersistence = persistence.then(
        () => undefined,
        () => undefined,
      );
      pendingPersistence = settledPersistence;
      try {
        persistedSignatures = await persistence;
      } catch (error) {
        console.warn("[listener] failed to persist meeting chat", error);
        return;
      } finally {
        if (pendingPersistence === settledPersistence) {
          pendingPersistence = null;
        }
      }
      if (stopped) {
        return;
      }
      if (!(await captureIsEnabled())) {
        baselineContext = null;
        return;
      }
      for (const signature of persistedSignatures) {
        rememberSignature(seenSignatures, signature);
      }
    } catch (error) {
      console.warn("[listener] failed to capture meeting chat", error);
    }
  };

  const capture = () => {
    if (stopped || inFlight) {
      return inFlight ?? Promise.resolve();
    }

    const pendingCapture = captureOnce().finally(() => {
      if (inFlight === pendingCapture) {
        inFlight = null;
      }
    });
    inFlight = pendingCapture;
    return pendingCapture;
  };

  void capture();
  const interval = setInterval(() => {
    void capture();
  }, MEETING_CHAT_CAPTURE_INTERVAL_MS);

  return async () => {
    stopped = true;
    clearInterval(interval);
    await pendingPersistence;
  };
}

function rememberSignature(signatures: Set<string>, signature: string) {
  signatures.delete(signature);
  signatures.add(signature);
  while (signatures.size > MAX_CAPTURED_CHAT_WINDOW) {
    const oldestSignature = signatures.values().next().value;
    if (oldestSignature === undefined) {
      break;
    }
    signatures.delete(oldestSignature);
  }
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
        message.text.length,
        hashMeetingChatText(message.text),
      ].join("\n");
}

function hashMeetingChatText(text: string) {
  let first = 0x811c9dc5;
  let second = 0x9e3779b9;
  for (let index = 0; index < text.length; index += 1) {
    const code = text.charCodeAt(index);
    first = Math.imul(first ^ code, 0x01000193);
    second = Math.imul(second ^ code, 0x85ebca6b);
  }
  return [first, second]
    .map((hash) => (hash >>> 0).toString(16).padStart(8, "0"))
    .join("");
}

function normalizeMessageText(text: string) {
  return text.replace(/\s+/g, " ").trim();
}
