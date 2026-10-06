import { decodeJwtPayload } from "../auth/billing.ts";
import {
  record,
  subscriptionJson as chatgptJson,
} from "./subscription-http.ts";

export const CHATGPT_BASE_URL = "https://chatgpt.com/backend-api/codex";
export const CHATGPT_DEVICE_URL = "https://auth.openai.com/codex/device";
const CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann";
const AUTH_URL = "https://auth.openai.com";

export type ChatgptCredential = {
  type: "oauth";
  access: string;
  refresh: string;
  expires: number;
  accountId: string;
};

export type ChatgptDeviceCode = {
  deviceAuthId: string;
  userCode: string;
  interval: number;
  expires: number;
};

function claims(token: string) {
  return record(decodeJwtPayload(token));
}

function accountId(token: string) {
  const payload = claims(token);
  return (
    record(payload["https://api.openai.com/auth"]).chatgpt_account_id ??
    payload.chatgpt_account_id
  );
}

export function parseChatgptCredential(
  raw: string | null,
): ChatgptCredential | null {
  try {
    const value = record(JSON.parse(raw ?? "null"));
    if (
      value.type !== "oauth" ||
      ![value.access, value.refresh, value.accountId].every(
        (field) => typeof field === "string" && /^[\x21-\x7e]+$/.test(field),
      ) ||
      typeof value.expires !== "number" ||
      !Number.isFinite(value.expires)
    )
      return null;
    return value as ChatgptCredential;
  } catch {
    return null;
  }
}

export function chatgptHeaders(
  access: string,
  savedAccountId?: string,
): Record<string, string> {
  const id = savedAccountId ?? accountId(access);
  if (typeof id !== "string" || !id || /[\r\n]/.test(id))
    throw new Error("Reconnect ChatGPT in Settings.");
  const residency = record(
    claims(access)["https://api.openai.com/auth"],
  ).chatgpt_compute_residency;
  return {
    Authorization: `Bearer ${access}`,
    "ChatGPT-Account-ID": id,
    originator: "codex_cli_rs",
    "OpenAI-Beta": "responses=experimental",
    "User-Agent": "codex_cli_rs",
    ...(typeof residency === "string" &&
    residency !== "no_constraint" &&
    !/[\r\n]/.test(residency)
      ? { "x-openai-internal-codex-residency": residency }
      : {}),
  };
}

async function post(
  path: string,
  body: Record<string, string>,
  fetcher: typeof fetch,
  signal?: AbortSignal,
  form = false,
) {
  const result = await chatgptJson(
    `${AUTH_URL}${path}`,
    {
      method: "POST",
      headers: {
        "Content-Type": form
          ? "application/x-www-form-urlencoded"
          : "application/json",
      },
      body: form ? new URLSearchParams(body).toString() : JSON.stringify(body),
      signal,
    },
    fetcher,
    15_000,
  );
  if (path.endsWith("/deviceauth/token") && [403, 404].includes(result.status))
    return result;
  if (result.status >= 400) {
    throw new Error(
      result.status === 429
        ? "ChatGPT is rate limiting sign-in. Try again shortly."
        : "Couldn’t connect ChatGPT. Enable device code login in ChatGPT Settings → Security, then try again.",
    );
  }
  return result;
}

function credential(
  json: Record<string, unknown>,
  previous?: ChatgptCredential,
): ChatgptCredential {
  const access = json.access_token;
  const expiresIn = Number(json.expires_in ?? 3600);
  const parsed = parseChatgptCredential(
    JSON.stringify({
      type: "oauth",
      access,
      refresh: json.refresh_token ?? previous?.refresh,
      expires: Date.now() + expiresIn * 1000,
      accountId:
        (typeof json.id_token === "string"
          ? accountId(json.id_token)
          : undefined) ??
        (typeof access === "string" ? accountId(access) : undefined) ??
        previous?.accountId,
    }),
  );
  if (!parsed || expiresIn <= 0)
    throw new Error(
      "ChatGPT returned an invalid connection. Try signing in again.",
    );
  return parsed;
}

export async function requestChatgptDeviceCode(
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<ChatgptDeviceCode> {
  const { json } = await post(
    "/api/accounts/deviceauth/usercode",
    { client_id: CLIENT_ID },
    fetcher,
    signal,
  );
  const userCode = json.user_code ?? json.usercode;
  const interval = Number(json.interval ?? 5);
  if (
    typeof json.device_auth_id !== "string" ||
    !json.device_auth_id ||
    typeof userCode !== "string" ||
    !userCode ||
    !Number.isFinite(interval) ||
    interval < 0
  )
    throw new Error("ChatGPT returned an invalid sign-in code. Try again.");
  return {
    deviceAuthId: json.device_auth_id,
    userCode,
    interval: Math.max(1, interval) * 1000,
    expires: Date.now() + 15 * 60 * 1000,
  };
}

export async function pollChatgptDeviceCode(
  session: ChatgptDeviceCode,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<ChatgptCredential | null> {
  if (Date.now() >= session.expires)
    throw new Error("This ChatGPT sign-in code expired. Start again.");
  const { status, json } = await post(
    "/api/accounts/deviceauth/token",
    { device_auth_id: session.deviceAuthId, user_code: session.userCode },
    fetcher,
    signal,
  );
  if ([403, 404].includes(status)) return null;
  if (
    typeof json.authorization_code !== "string" ||
    !json.authorization_code ||
    typeof json.code_verifier !== "string" ||
    !json.code_verifier
  )
    throw new Error(
      "ChatGPT returned an invalid sign-in response. Start again.",
    );
  const result = await post(
    "/oauth/token",
    {
      grant_type: "authorization_code",
      client_id: CLIENT_ID,
      code: json.authorization_code,
      code_verifier: json.code_verifier,
      redirect_uri: `${AUTH_URL}/deviceauth/callback`,
    },
    fetcher,
    signal,
    true,
  );
  return credential(result.json);
}

export async function refreshChatgptCredential(
  previous: ChatgptCredential,
  fetcher: typeof fetch,
): Promise<ChatgptCredential> {
  try {
    const { json } = await post(
      "/oauth/token",
      {
        grant_type: "refresh_token",
        client_id: CLIENT_ID,
        refresh_token: previous.refresh,
      },
      fetcher,
      undefined,
      true,
    );
    return credential(json, previous);
  } catch {
    throw new Error("Couldn’t refresh ChatGPT. Reconnect ChatGPT in Settings.");
  }
}

export async function listChatgptModels(
  access: string,
  savedAccountId: string | undefined,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<string[]> {
  const { status, json } = await chatgptJson(
    `${CHATGPT_BASE_URL}/models?client_version=0.145.0`,
    {
      headers: chatgptHeaders(access, savedAccountId),
      signal,
    },
    fetcher,
    8_000,
  );
  if (status >= 400) {
    throw new Error(
      status === 401 || status === 403
        ? "Reconnect ChatGPT in Settings."
        : "Couldn’t load ChatGPT models. Try again.",
    );
  }
  if (!Array.isArray(json.models))
    throw new Error("ChatGPT returned an invalid model list.");
  return [
    ...new Set(
      json.models.flatMap((entry) => {
        const model = record(entry);
        return model.visibility !== "hide" &&
          typeof model.slug === "string" &&
          model.slug.trim() &&
          model.slug.length <= 200 &&
          !/[\r\n]/.test(model.slug)
          ? [model.slug]
          : [];
      }),
    ),
  ];
}
