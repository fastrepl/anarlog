import { Platform } from "react-native";

import LocalSpeech from "../../modules/local-speech";
import { throwIfAborted } from "../lib/abort.ts";
import { localSpeechModel } from "../settings/local-speech-models.ts";

const errors: Record<string, string> = {
  ERR_MLKIT_UNSUPPORTED_DEVICE:
    "Google on-device transcription is unavailable on this phone. Choose another transcription provider.",
  ERR_MLKIT_BUSY:
    "Google is still transcribing another recording. This recording is saved for transcription afterward.",
  ERR_MLKIT_STREAM_FAILED:
    "Google stopped the live stream. Your recording is saved for transcription after you stop.",
  ERR_MLKIT_MODEL_REQUIRED:
    "Download the speech model in Settings > Transcription provider > Google on-device.",
  ERR_MLKIT_MODEL_DOWNLOADING:
    "Google's speech model is downloading. Retry when the download finishes.",
  ERR_MLKIT_MODEL_UNAVAILABLE:
    "The selected speech engine or language is unavailable. Update Google's speech services and Android AICore, then check again.",
  ERR_MLKIT_FOREGROUND_REQUIRED:
    "Keep Anarlog open while downloading the speech model.",
  ERR_MLKIT_BACKGROUND_USE_BLOCKED:
    "Google blocked background transcription. Return to Anarlog to recover the saved audio, or choose another on-device engine.",
  ERR_MLKIT_INTERRUPTED:
    "Local transcription was interrupted. Keep Anarlog open and retry. Your recording is saved.",
  ERR_MLKIT_AUDIO_FORMAT:
    "Google on-device currently supports Anarlog recordings and 16 kHz mono PCM WAV files. Choose another provider for this imported format.",
  ERR_MLKIT_AUDIO_MISSING: "The recording could not be found on this device.",
  ERR_MLKIT_RESPONSE_TOO_LARGE: "The local transcript is too large to save.",
  ERR_MLKIT_INCOMPLETE:
    "Google stopped before finishing the recording. Your recording is saved; retry transcription.",
  ERR_MLKIT_NATIVE_SEGMENTED_REQUIRED:
    "This phone’s on-device recognizer does not support continuous audio input. Choose ML Kit or retry the saved recording with another engine.",
  ERR_MLKIT_EMPTY:
    "Google did not recognize any speech. Your recording is saved; you can retry transcription.",
};

export function localSpeechLocale(language: string): string {
  const defaults: Record<string, string> = {
    ar: "ar-SA",
    da: "da-DK",
    de: "de-DE",
    en: "en-US",
    es: "es-ES",
    fr: "fr-FR",
    hi: "hi-IN",
    id: "id-ID",
    it: "it-IT",
    ja: "ja-JP",
    ko: "ko-KR",
    nl: "nl-NL",
    pl: "pl-PL",
    pt: "pt-PT",
    ru: "ru-RU",
    sv: "sv-SE",
    th: "th-TH",
    tr: "tr-TR",
    vi: "vi-VN",
    zh: "cmn-Hans-CN",
  };
  return defaults[language] ?? language;
}

async function withLocalSpeech<T>(
  operation: (module: NonNullable<typeof LocalSpeech>) => Promise<T>,
  model = "mlkit-advanced",
) {
  if (Platform.OS !== "android" || !LocalSpeech)
    throw Object.assign(
      new Error(
        "Install an Android build with Google on-device speech recognition.",
      ),
      {
        code: "ERR_MLKIT_BUILD_REQUIRED",
      },
    );
  try {
    localSpeechModel(model);
    if (!isLocalSpeechSupportedOS(model))
      throw { code: "ERR_MLKIT_UNSUPPORTED_DEVICE" };
    return await operation(LocalSpeech);
  } catch (error) {
    const code =
      error && typeof error === "object" && "code" in error
        ? String(error.code)
        : "ERR_MLKIT_FAILED";
    throw Object.assign(
      new Error(
        errors[code] ??
          "Google on-device transcription failed. Check the model in Settings and retry. Your recording is saved.",
      ),
      { code },
    );
  }
}

export function isLocalSpeechSupportedOS(model = "mlkit-advanced") {
  return (
    Platform.OS === "android" &&
    Number(Platform.Version) >= (model === "android-on-device" ? 33 : 31)
  );
}

export async function checkLocalSpeechStatus(
  language: string,
  model = "mlkit-advanced",
) {
  if (!isLocalSpeechSupportedOS(model) || !LocalSpeech)
    return "unavailable" as const;
  return withLocalSpeech(
    (module) => module.checkStatus(localSpeechLocale(language), model),
    model,
  );
}

export function downloadLocalSpeechModel(
  language: string,
  model = "mlkit-advanced",
) {
  return withLocalSpeech(
    (module) => module.downloadModel(localSpeechLocale(language), model),
    model,
  );
}

export function transcribeLocalFile(
  uri: string,
  language: string,
  model = "mlkit-advanced",
) {
  return withLocalSpeech(
    (module) => module.transcribeFile(uri, localSpeechLocale(language), model),
    model,
  );
}

export async function startLocalLiveTranscription({
  sessionId,
  language,
  model = "mlkit-advanced",
  sampleRate,
  channels,
  onMessage,
  onError,
  signal,
}: {
  sessionId: string;
  language: string;
  model?: string;
  sampleRate: number;
  channels: number;
  onMessage: (message: string) => void;
  onError: (error: Error) => void;
  signal: AbortSignal;
}) {
  if (sampleRate !== 16_000 || channels !== 1)
    throw new Error("Local live transcription requires 16 kHz mono audio.");
  return withLocalSpeech(async (module) => {
    throwIfAborted(signal);
    let cancelled = false;
    let finishing = false;
    let terminalResolve: (complete: boolean) => void;
    const terminal = new Promise<boolean>((resolve) => {
      terminalResolve = resolve;
    });
    const subscription = module.addListener("onSpeechResult", (event) => {
      if (cancelled || event.sessionId !== sessionId) return;
      if (event.type === "error" || event.type === "completed") {
        terminalResolve(event.type === "completed" && finishing);
        if (event.type === "error" || !finishing)
          onError(
            new Error(
              errors[event.code ?? "ERR_MLKIT_STREAM_FAILED"] ??
                errors.ERR_MLKIT_STREAM_FAILED,
            ),
          );
        return;
      }
      onMessage(
        JSON.stringify({
          type: "Results",
          is_final: event.type === "final",
          start: (event.startMs ?? 0) / 1000,
          duration:
            Math.max(0, (event.endMs ?? 0) - (event.startMs ?? 0)) / 1000,
          channel: {
            alternatives: [{ transcript: event.text ?? "", words: [] }],
          },
        }),
      );
    });
    const cancel = () => {
      if (cancelled) return;
      cancelled = true;
      subscription.remove();
      signal.removeEventListener("abort", cancel);
      module.cancelLive(sessionId);
      terminalResolve(false);
    };
    signal.addEventListener("abort", cancel, { once: true });
    try {
      await module.startLive(sessionId, localSpeechLocale(language), model);
      if (cancelled) {
        module.cancelLive(sessionId);
        throw new Error("Local live transcription was cancelled.");
      }
    } catch (error) {
      cancel();
      throw error;
    }
    return {
      cancel,
      sendAudio(buffer: ArrayBuffer) {
        if (cancelled || !module.sendAudio(sessionId, new Uint8Array(buffer)))
          throw new Error(
            "Local speech recognition could not keep up with the recording.",
          );
      },
      async finish() {
        if (cancelled) return false;
        finishing = true;
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
          return await Promise.race([
            Promise.all([module.finishLive(sessionId), terminal]).then(
              ([native, delivered]) => native && delivered,
            ),
            new Promise<boolean>((resolve) => {
              timer = setTimeout(() => resolve(false), 95_000);
            }),
          ]);
        } finally {
          clearTimeout(timer);
          cancel();
        }
      },
    };
  }, model);
}
