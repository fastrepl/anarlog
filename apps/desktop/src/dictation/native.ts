import { Channel } from "@tauri-apps/api/core";

import {
  commands as dictation,
  type DictationError,
  type DictationUpdate,
} from "@anlg/plugin-dictation";

import { DictationRetired, type DictationNative } from "./engine";

type Result<T> =
  | { status: "ok"; data: T }
  | { status: "error"; error: DictationError };

const MESSAGES: Partial<Record<DictationError["code"], string>> = {
  busy: "Voice input is already in use. Finish the other dictation first.",
  capture:
    "Couldn't open the microphone. Check the selected input device and try again.",
  recording: "Recording failed. Please try again.",
  recoveryNotFound: "This dictation is no longer available.",
};

export function unwrapDictation<T>(result: Result<T>): T {
  if (result.status === "ok") return result.data;
  const { code, message } = result.error;
  // Stale and cancelled operations end quietly: their results belong to nobody.
  if (code === "stale" || code === "cancelled") throw new DictationRetired();
  throw new Error(MESSAGES[code] ?? message);
}

export const nativeDictation: DictationNative = {
  begin: async (request, onUpdate) =>
    unwrapDictation(
      await dictation.beginDictation(
        request,
        new Channel<DictationUpdate>(onUpdate),
      ),
    ),
  attach: async (operationId, stream) => {
    unwrapDictation(await dictation.attachDictationStream(operationId, stream));
  },
  finish: async (operationId) =>
    unwrapDictation(await dictation.finishDictation(operationId)),
  complete: async (operationId, batchText) =>
    unwrapDictation(await dictation.completeDictation(operationId, batchText)),
  release: async (operationId, keep) =>
    unwrapDictation(await dictation.releaseDictation(operationId, keep)),
  cancel: async (operationId) => {
    unwrapDictation(await dictation.cancelDictation(operationId));
  },
  saveRecoveryText: async (accountId, id, text) => {
    unwrapDictation(
      await dictation.saveDictationRecoveryText(accountId, id, text),
    );
  },
};
