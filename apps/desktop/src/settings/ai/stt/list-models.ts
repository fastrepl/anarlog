import { providerFetch } from "~/ai/provider-fetch";

const LISTABLE_REMOTE_STT_PROVIDERS = new Set([
  "deepgram",
  "soniox",
  "elevenlabs",
  "openai",
  "groq",
  "mistral",
  "fireworks",
  "together",
  "siliconflow",
  "dashscope",
  "google_generative_ai",
  "cloudflare_workers_ai",
]);

export function isListableRemoteSttProvider(providerId: string): boolean {
  return LISTABLE_REMOTE_STT_PROVIDERS.has(providerId);
}

type ProviderRequestSpec = {
  url: string;
  headers: Record<string, string>;
  parse: (body: unknown) => string[];
};

function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

function arrayOfObjects(value: unknown): Record<string, unknown>[] {
  return Array.isArray(value) ? value.map(record) : [];
}

function dedupeStrings(values: Array<string | null | undefined>): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of values) {
    const value = typeof raw === "string" ? raw.trim() : "";
    if (!value || seen.has(value)) {
      continue;
    }
    seen.add(value);
    result.push(value);
  }
  return result;
}

function extractOpenAiIds(
  body: unknown,
  predicate?: (id: string, entry: Record<string, unknown>) => boolean,
): string[] {
  const items = Array.isArray(body)
    ? arrayOfObjects(body)
    : arrayOfObjects(record(body).data);
  return dedupeStrings(
    items.flatMap((entry) => {
      const id = typeof entry.id === "string" ? entry.id.trim() : "";
      if (!id) {
        return [];
      }
      return !predicate || predicate(id, entry) ? [id] : [];
    }),
  );
}

function buildRequestSpec(
  providerId: string,
  baseUrl: string,
  apiKey: string,
): ProviderRequestSpec | null {
  const normalizedBase = baseUrl.trim().replace(/\/+$/, "");
  const trimmedKey = apiKey.trim();
  if (!normalizedBase || !trimmedKey) {
    return null;
  }

  let parsedOrigin: string;
  try {
    parsedOrigin = new URL(normalizedBase).origin;
  } catch {
    return null;
  }

  switch (providerId) {
    case "deepgram":
      return {
        url: normalizedBase.endsWith("/v1")
          ? `${normalizedBase}/models`
          : `${normalizedBase}/v1/models`,
        headers: { Authorization: `Token ${trimmedKey}` },
        parse: (body) =>
          dedupeStrings(
            arrayOfObjects(record(body).stt).map((entry) =>
              typeof entry.canonical_name === "string" &&
              entry.canonical_name.trim()
                ? entry.canonical_name
                : typeof entry.name === "string"
                  ? entry.name
                  : "",
            ),
          ),
      };

    case "soniox":
      return {
        url: `${normalizedBase.replace(/\/v1$/, "")}/v1/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          dedupeStrings(
            arrayOfObjects(record(body).models).map((entry) => {
              const id = typeof entry.id === "string" ? entry.id.trim() : "";
              return /^stt-async-/i.test(id) ? "" : id;
            }),
          ),
      };

    case "elevenlabs":
      return {
        url: `${normalizedBase.replace(/\/v1$/, "")}/v1/models`,
        headers: { "xi-api-key": trimmedKey },
        parse: (body) => {
          const items = Array.isArray(body)
            ? arrayOfObjects(body)
            : arrayOfObjects(record(body).models);
          return dedupeStrings(
            items.flatMap((entry) => {
              const id =
                typeof entry.model_id === "string" ? entry.model_id.trim() : "";
              if (!id) {
                return [];
              }
              if (
                entry.can_do_text_to_speech === true &&
                !id.toLowerCase().includes("scribe")
              ) {
                return [];
              }
              return [id];
            }),
          );
        },
      };

    case "openai":
      return {
        url: `${normalizedBase}/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(body, (id) => /^(whisper|.*transcribe)/i.test(id)),
      };

    case "groq":
      return {
        url: `${normalizedBase}/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(body, (id) => /whisper|asr|transcribe/i.test(id)),
      };

    case "mistral":
      return {
        url: `${normalizedBase}/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(body, (id, entry) => {
            if (
              Object.prototype.hasOwnProperty.call(
                entry,
                "audio_transcription_capabilities",
              )
            ) {
              const caps = entry.audio_transcription_capabilities;
              if (caps === null || caps === false || caps === undefined) {
                return false;
              }
              return Array.isArray(caps) ? caps.length > 0 : true;
            }
            const capabilities = record(entry.capabilities);
            if (typeof capabilities.audio_transcription === "boolean") {
              return capabilities.audio_transcription;
            }
            if (/(?:^|[-_/])tts(?:$|[-_/])|text-to-speech/i.test(id)) {
              return false;
            }
            return /voxtral|transcribe|asr/i.test(id);
          }),
      };

    case "fireworks":
      return {
        url: `${parsedOrigin}/inference/v1/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(body, (id) =>
            /whisper|asr|transcribe|speech|audio/i.test(id),
          ),
      };

    case "together":
      return {
        url: `${normalizedBase}/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(
            body,
            (id, entry) =>
              entry.type === "audio" ||
              /whisper|asr|transcribe|speech/i.test(id),
          ),
      };

    case "siliconflow":
      return {
        url: `${normalizedBase}/models?sub_type=audio`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) => extractOpenAiIds(body),
      };

    case "dashscope":
      return {
        url: `${normalizedBase.replace(/\/compatible-mode\/v1$/, "")}/compatible-mode/v1/models`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) =>
          extractOpenAiIds(body, (id) =>
            /asr|transcribe|sensevoice|paraformer|whisper|audio/i.test(id),
          ),
      };

    case "google_generative_ai":
      return {
        url: `${normalizedBase}/models`,
        headers: { "x-goog-api-key": trimmedKey },
        parse: (body) =>
          dedupeStrings(
            arrayOfObjects(record(body).models).flatMap((entry) => {
              const rawName =
                typeof entry.name === "string" ? entry.name.trim() : "";
              const id = rawName.replace(/^models\//, "");
              return /gemini.*(transcribe|live)/i.test(id) ? [id] : [];
            }),
          ),
      };

    case "cloudflare_workers_ai":
      return {
        url: `${normalizedBase.replace(/\/v1$/i, "")}/models/search?task=Automatic%20Speech%20Recognition`,
        headers: { Authorization: `Bearer ${trimmedKey}` },
        parse: (body) => {
          const obj = record(body);
          const items = arrayOfObjects(
            Array.isArray(obj.result) ? obj.result : obj.data,
          );
          return dedupeStrings(
            items.map((entry) => {
              const id = (
                typeof entry.name === "string" && entry.name.trim()
                  ? entry.name
                  : typeof entry.id === "string"
                    ? entry.id
                    : ""
              ).trim();
              return /^@cf\/deepgram\/(?:nova-3|flux)$/i.test(id) ? id : "";
            }),
          );
        },
      };

    default:
      return null;
  }
}

export async function listSttProviderModels(
  providerId: string,
  baseUrl: string,
  apiKey: string,
  options?: { signal?: AbortSignal },
): Promise<string[]> {
  const spec = buildRequestSpec(providerId, baseUrl, apiKey);
  if (!spec) {
    return [];
  }

  try {
    const response = await providerFetch(spec.url, {
      headers: spec.headers,
      signal: options?.signal,
    });
    if (!response.ok) {
      return [];
    }
    return spec.parse(await response.json());
  } catch {
    return [];
  }
}
