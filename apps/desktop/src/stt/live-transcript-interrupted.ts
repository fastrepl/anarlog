import type { DegradedError } from "@anlg/plugin-transcription";

import { PROVIDERS } from "~/settings/ai/stt/shared";

const SAVED_AUDIO_NOTE =
  "Your audio is still being saved, and the missing part will be filled in from the recording.";

const LOCAL_PROVIDER_IDS = new Set(["soniqo", "apple_speech", "local_file"]);

function providerDisplayName(providerId: string | null | undefined) {
  if (!providerId) return null;
  return (
    PROVIDERS.find((provider) => provider.id === providerId)?.displayName ??
    null
  );
}

function isProviderSideStreamError(message: string) {
  return (
    /provider error/i.test(message) ||
    /code=Some\(4\d{3}\)/.test(message) ||
    /^[\w-]+: /.test(message)
  );
}

function getPauseCause({
  degraded,
  sttProvider,
  online,
}: {
  degraded: DegradedError | null;
  sttProvider: string | null | undefined;
  online: boolean;
}) {
  const isCloud = sttProvider === "anarlog";
  const providerName = isCloud
    ? null
    : (providerDisplayName(sttProvider) ?? "the transcription service");

  const isLocal = !!sttProvider && LOCAL_PROVIDER_IDS.has(sttProvider);

  if (!online && !isLocal) {
    return "you're offline";
  }

  if (degraded?.type === "authentication_failed") {
    return isCloud
      ? "Anarlog couldn't verify your account"
      : `${providerName} rejected your API key`;
  }

  if (degraded?.type === "provider_configuration") {
    return isCloud
      ? "our speech-to-text provider rejected the request"
      : `${providerName} rejected the transcription settings`;
  }

  if (
    degraded?.type === "stream_error" &&
    isProviderSideStreamError(degraded.message)
  ) {
    return isCloud
      ? "our speech-to-text provider is having an outage"
      : `${providerName} is having an outage`;
  }

  if (degraded) {
    if (isCloud) {
      return "Anarlog's transcription server is having issues";
    }
    if (sttProvider && LOCAL_PROVIDER_IDS.has(sttProvider)) {
      return "the local transcription model stopped responding";
    }
    return `Anarlog can't reach ${providerName}`;
  }

  return isCloud
    ? "Anarlog's transcription server stopped sending words"
    : `${providerName} stopped sending words`;
}

export function getLiveTranscriptPausedMessage({
  degraded,
  sttProvider,
  online = typeof navigator === "undefined" ? true : navigator.onLine,
}: {
  degraded: DegradedError | null;
  sttProvider: string | null | undefined;
  online?: boolean;
}) {
  const cause = getPauseCause({ degraded, sttProvider, online });
  return `Live transcript paused because ${cause}. ${SAVED_AUDIO_NOTE}`;
}
