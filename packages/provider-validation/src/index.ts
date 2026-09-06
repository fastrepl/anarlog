import { sha256 } from "js-sha256";

export type ProviderCredential = {
  provider: string;
  baseUrl: string;
  apiKey: string;
};

type CredentialFetch = (url: string, init: RequestInit) => Promise<Response>;
const verified = new WeakMap<CredentialFetch, Map<string, number>>();

export class ProviderCredentialError extends Error {
  readonly retryable: boolean;

  constructor(message: string, retryable = false) {
    super(message);
    this.retryable = retryable;
  }
}

export function providerCredentialIdentity(credential: ProviderCredential) {
  return sha256(JSON.stringify(credential));
}

export async function verifyProviderCredentials(
  credential: ProviderCredential,
  fetcher: CredentialFetch,
  signal?: AbortSignal,
): Promise<void> {
  const apiKey = credential.apiKey.trim();
  if (!apiKey || apiKey.length > 8192 || /[\r\n]/.test(apiKey))
    throw new ProviderCredentialError("Enter a valid API key.");
  let base: URL;
  try {
    base = new URL(credential.baseUrl);
  } catch {
    throw new ProviderCredentialError("Enter a valid base URL.");
  }
  if (base.username || base.password || base.search || base.hash)
    throw new ProviderCredentialError(
      "Enter a base URL without credentials or query parameters.",
    );
  if (
    base.protocol !== "https:" &&
    !(
      base.protocol === "http:" &&
      ["localhost", "127.0.0.1", "[::1]"].includes(base.hostname)
    )
  )
    throw new ProviderCredentialError("Use HTTPS for provider credentials.");

  signal?.throwIfAborted();
  const identity = providerCredentialIdentity({ ...credential, apiKey });
  const recent = verified.get(fetcher) ?? new Map<string, number>();
  verified.set(fetcher, recent);
  if ((recent.get(identity) ?? 0) > Date.now()) return;

  const controller = new AbortController();
  const abort = () => controller.abort();
  signal?.throwIfAborted();
  signal?.addEventListener("abort", abort, { once: true });
  const timer = setTimeout(abort, 8_000);
  try {
    const request = credentialRequest({ ...credential, apiKey });
    const response = await fetcher(request.url, {
      ...request.init,
      signal: controller.signal,
      redirect: "error",
    });
    if (response.status === 401 || response.status === 403) {
      await response.body?.cancel();
      throw new ProviderCredentialError(
        "The provider rejected this key or its permissions. Check the key and try again.",
      );
    }
    if (!response.ok) {
      await response.body?.cancel();
      throw new ProviderCredentialError(
        response.status === 429
          ? "The provider is rate limiting verification. Try again shortly."
          : "Couldn’t verify this key with the provider. Check the connection and try again.",
        true,
      );
    }
    const body = await readResponse(response, request.plainText);
    if (!request.accept(body))
      throw new ProviderCredentialError(
        "The provider did not confirm this key. Check the key and connection.",
      );

    // A public model catalog cannot prove that credentials work. Gateways must
    // also reject an invalid credential before their model response is trusted.
    if (request.checkAuthentication) {
      const control = credentialRequest({
        ...credential,
        apiKey: "anarlog-invalid-key-verification",
      });
      const rejected = await fetcher(control.url, {
        ...control.init,
        signal: controller.signal,
        redirect: "error",
      });
      await rejected.body?.cancel();
      if (rejected.status !== 401 && rejected.status !== 403) {
        if (!rejected.ok)
          throw new ProviderCredentialError(
            rejected.status === 429
              ? "The provider is rate limiting verification. Try again shortly."
              : "Couldn’t verify this key with the provider. Check the connection and try again.",
            true,
          );
        throw new ProviderCredentialError(
          "This endpoint doesn’t support API key verification. Use an authenticated model-list endpoint.",
        );
      }
    }
    controller.signal.throwIfAborted();
    // Saving and refreshing availability use the same proof, without retaining
    // raw credentials or turning a temporary network failure into a lost setup.
    recent.set(identity, Date.now() + 60_000);
    if (recent.size > 128) recent.delete(recent.keys().next().value!);
  } catch (error) {
    if (error instanceof ProviderCredentialError) throw error;
    throw new ProviderCredentialError(
      "Couldn’t verify this key. Check your connection and try again.",
      true,
    );
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
  }
}

function credentialRequest({ provider, baseUrl, apiKey }: ProviderCredential) {
  const base = baseUrl.replace(/\/+$/, "");
  const origin = new URL(base).origin;
  let url = `${base}/models`;
  let headers: Record<string, string> = { Authorization: `Bearer ${apiKey}` };
  let method = "GET";
  let body: string | undefined;
  let checkAuthentication = false;
  let plainText = false;
  let accept = (value: unknown) => {
    const data = record(value);
    return (
      !data.error && (Array.isArray(data.data) || Array.isArray(data.models))
    );
  };
  switch (provider) {
    case "anthropic":
      checkAuthentication = new URL(base).hostname !== "api.anthropic.com";
      headers = { "x-api-key": apiKey, "anthropic-version": "2023-06-01" };
      break;
    case "google_generative_ai":
      checkAuthentication =
        new URL(base).hostname !== "generativelanguage.googleapis.com";
      headers = { "x-goog-api-key": apiKey };
      break;
    case "azure_openai":
      url = `${base.replace(/\/openai(?:\/v1)?$/, "")}/openai/models?api-version=2024-10-21`;
      headers = { "api-key": apiKey };
      break;
    case "azure_ai":
      headers = { "api-key": apiKey };
      break;
    case "azure_speech":
      url = `${origin}/sts/v1.0/issueToken`;
      method = "POST";
      headers = { "Ocp-Apim-Subscription-Key": apiKey };
      plainText = true;
      accept = (value) =>
        typeof value === "string" && value.split(".").length === 3;
      break;
    case "google_cloud":
      url = `${base}/operations?pageSize=1`;
      checkAuthentication = new URL(base).hostname !== "speech.googleapis.com";
      accept = (value) =>
        value !== null &&
        typeof value === "object" &&
        !Array.isArray(value) &&
        !record(value).error &&
        (record(value).operations === undefined ||
          Array.isArray(record(value).operations));
      break;
    case "google_vertex_ai":
      url = `${origin}/v1beta1/publishers/google/models?pageSize=1`;
      accept = (value) => Array.isArray(record(value).publisherModels);
      checkAuthentication = true;
      break;
    case "openrouter":
      url = `${base}/key`;
      accept = (value) => typeof record(record(value).data).label === "string";
      break;
    case "deepgram":
      url = `${base}/projects`;
      headers = { Authorization: `Token ${apiKey}` };
      accept = (value) => Array.isArray(record(value).projects);
      break;
    case "assemblyai":
      url = `${base}/v2/transcript?limit=1`;
      headers = { Authorization: apiKey };
      accept = (value) => Array.isArray(record(value).transcripts);
      break;
    case "siliconflow":
      url = `${base}/user/info`;
      accept = (value) => record(value).status === true;
      break;
    case "xai":
      url = `${base}/api-key`;
      accept = (value) =>
        record(value).api_key_blocked === false &&
        record(value).api_key_disabled === false;
      break;
    case "cloudflare_workers_ai":
      url = `${origin}/client/v4/user/tokens/verify`;
      accept = (value) =>
        record(value).success === true &&
        record(record(value).result).status === "active";
      break;
    case "cartesia":
      url = `${base}/voices?limit=1`;
      headers["Cartesia-Version"] = "2025-04-16";
      checkAuthentication = true;
      accept = (value) =>
        Array.isArray(value) || Array.isArray(record(value).data);
      break;
    case "smallestai":
      url = `${base}/waves/v1/voice-cloning`;
      checkAuthentication = true;
      accept = (value) => Array.isArray(record(value).data);
      break;
    case "fireworks":
      url = `${origin}/inference/v1/models`;
      checkAuthentication = true;
      break;
    case "deepseek":
      url = `${base}/user/balance`;
      accept = (value) => typeof record(value).is_available === "boolean";
      break;
    case "cohere":
      url = `${origin}/v1/check-api-key`;
      method = "POST";
      headers["Content-Type"] = "application/json";
      body = "{}";
      accept = (value) => record(value).valid === true;
      break;
    case "soniox":
      url = `${base}/v1/transcriptions?limit=1`;
      accept = (value) => Array.isArray(record(value).transcriptions);
      break;
    case "speechmatics":
      url = `${base}/jobs`;
      accept = (value) => Array.isArray(record(value).jobs);
      break;
    case "revai":
      url = `${base}/account`;
      accept = (value) => typeof record(value).id === "string";
      break;
    case "elevenlabs":
      url = `${base}/v1/user`;
      headers = { "xi-api-key": apiKey };
      accept = (value) => typeof record(value).user_id === "string";
      break;
    case "gladia":
      url = `${base}/v2/live?limit=1`;
      headers = { "x-gladia-key": apiKey };
      accept = (value) => Array.isArray(record(value).items);
      break;
    case "pyannote":
      url = `${base}/v1/test`;
      accept = (value) => record(value).status === "OK";
      break;
    case "dashscope":
      url = `${base}/compatible-mode/v1/models`;
      checkAuthentication = true;
      break;
    case "openai":
      checkAuthentication = new URL(base).hostname !== "api.openai.com";
      break;
    case "groq":
      checkAuthentication = new URL(base).hostname !== "api.groq.com";
      break;
    case "mistral":
      checkAuthentication = new URL(base).hostname !== "api.mistral.ai";
      break;
    default:
      checkAuthentication = true;
  }
  return {
    url,
    init: { method, headers, body },
    accept,
    checkAuthentication,
    plainText,
  };
}

function record(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

async function readResponse(
  response: Response,
  plainText: boolean,
): Promise<unknown> {
  const limit = 8 * 1024 * 1024;
  const reader = response.body?.getReader();
  if (!reader) throw new Error("Empty verification response");
  let text = "";
  let size = 0;
  const decoder = new TextDecoder();
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) {
        await reader.cancel();
        throw new Error("Verification response too large");
      }
      text += decoder.decode(value, { stream: true });
    }
    text += decoder.decode();
    return plainText ? text : JSON.parse(text);
  } finally {
    reader.releaseLock();
  }
}
