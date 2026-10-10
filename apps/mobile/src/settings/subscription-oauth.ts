import {
  CHATGPT_DEVICE_URL,
  listChatgptModels,
  parseChatgptCredential,
  pollChatgptDeviceCode,
  refreshChatgptCredential,
  requestChatgptDeviceCode,
} from "./chatgpt-oauth.ts";
import { defaultProviderConfig } from "./providers-model.ts";
import { record, subscriptionJson } from "./subscription-http.ts";

export type OAuthSubscriptionProvider =
  | "chatgpt"
  | "claude"
  | "grok"
  | "github_copilot";
export type SubscriptionCredential = {
  type: "oauth";
  access: string;
  refresh: string;
  expires: number;
  accountId?: string;
};
export type SubscriptionSession =
  | {
      kind: "device";
      url: string;
      userCode: string;
      deviceCode: string;
      interval: number;
      expires: number;
      notBefore: number;
    }
  | {
      kind: "code";
      url: string;
      verifier: string;
      state: string;
      expires: number;
    };

const CLAUDE_CLIENT = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const CLAUDE_TOKEN_URL = "https://platform.claude.com/v1/oauth/token";
const CLAUDE_REDIRECT = "https://platform.claude.com/oauth/code/callback";
const COPILOT_CLIENT = "Iv1.b507a08c87ecfe98";
const GROK_CLIENT = "b1a00492-073a-47ea-816f-4c329264a828";
const GROK_TOKEN_URL = "https://auth.x.ai/oauth2/token";

export const COPILOT_HEADERS = {
  "User-Agent": "GitHubCopilotChat/0.26.7",
  "Editor-Version": "vscode/1.99.3",
  "Editor-Plugin-Version": "copilot-chat/0.26.7",
  "Copilot-Integration-Id": "vscode-chat",
};
export const CLAUDE_HEADERS = {
  "anthropic-version": "2023-06-01",
  "anthropic-beta": "oauth-2025-04-20,interleaved-thinking-2025-05-14",
};
export const CLAUDE_CODE_IDENTITY =
  "You are Claude Code, Anthropic's official CLI for Claude.";

export function parseSubscriptionCredential(
  provider: OAuthSubscriptionProvider,
  raw: string | null,
): SubscriptionCredential | null {
  if (provider === "chatgpt") return parseChatgptCredential(raw);
  try {
    const value = record(JSON.parse(raw ?? "null"));
    if (
      value.type !== "oauth" ||
      ![value.access, value.refresh].every(
        (field) => typeof field === "string" && /^[\x21-\x7e]+$/.test(field),
      ) ||
      typeof value.expires !== "number" ||
      !Number.isFinite(value.expires)
    )
      return null;
    return {
      type: "oauth",
      access: value.access as string,
      refresh: value.refresh as string,
      expires: value.expires,
    };
  } catch {
    return null;
  }
}

async function post(
  url: string,
  body: Record<string, string>,
  fetcher: typeof fetch,
  signal?: AbortSignal,
  form = false,
) {
  return subscriptionJson(
    url,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": form
          ? "application/x-www-form-urlencoded"
          : "application/json",
      },
      body: form ? new URLSearchParams(body).toString() : JSON.stringify(body),
      signal,
    },
    fetcher,
    15_000,
    true,
  );
}

function tokenCredential(
  provider: OAuthSubscriptionProvider,
  json: Record<string, unknown>,
  previous?: SubscriptionCredential,
) {
  const expiresIn = Number(json.expires_in ?? 3600);
  const value = parseSubscriptionCredential(
    provider,
    JSON.stringify({
      type: "oauth",
      access: json.access_token,
      refresh: json.refresh_token ?? previous?.refresh,
      expires: Date.now() + expiresIn * 1000,
    }),
  );
  if (!value || expiresIn <= 0)
    throw new Error(
      "The provider returned an invalid connection. Sign in again.",
    );
  return value;
}

export async function startSubscriptionConnect(
  provider: OAuthSubscriptionProvider,
  fetcher: typeof fetch,
  signal?: AbortSignal,
  pkce?: { verifier: string; challenge: string; state: string },
): Promise<SubscriptionSession> {
  signal?.throwIfAborted();
  if (provider === "claude") {
    if (!pkce) throw new Error("Start Claude sign-in again.");
    const params = new URLSearchParams({
      code: "true",
      client_id: CLAUDE_CLIENT,
      response_type: "code",
      redirect_uri: CLAUDE_REDIRECT,
      scope: "user:profile user:inference",
      code_challenge: pkce.challenge,
      code_challenge_method: "S256",
      state: pkce.state,
    });
    return {
      kind: "code",
      // Anthropic's authorization page rejects form-style '+' scope separators.
      url: `https://claude.ai/oauth/authorize?${params.toString().replace(/\+/g, "%20")}`,
      verifier: pkce.verifier,
      state: pkce.state,
      expires: Date.now() + 15 * 60_000,
    };
  }
  if (provider === "chatgpt") {
    const code = await requestChatgptDeviceCode(fetcher, signal);
    return {
      kind: "device",
      url: CHATGPT_DEVICE_URL,
      userCode: code.userCode,
      deviceCode: code.deviceAuthId,
      interval: code.interval,
      expires: code.expires,
      notBefore: Date.now() + code.interval,
    };
  }
  const copilot = provider === "github_copilot";
  const { status, json } = await post(
    copilot
      ? "https://github.com/login/device/code"
      : "https://auth.x.ai/oauth2/device/code",
    copilot
      ? { client_id: COPILOT_CLIENT, scope: "read:user" }
      : {
          client_id: GROK_CLIENT,
          scope:
            "openid profile email offline_access grok-cli:access api:access",
        },
    fetcher,
    signal,
    !copilot,
  );
  const url = json.verification_uri_complete ?? json.verification_uri;
  const interval = Number(json.interval ?? 5);
  const expiresIn = Number(json.expires_in ?? 900);
  if (
    status >= 400 ||
    typeof json.user_code !== "string" ||
    !json.user_code ||
    typeof json.device_code !== "string" ||
    !json.device_code ||
    typeof url !== "string" ||
    !Number.isFinite(interval) ||
    interval < 0 ||
    !Number.isFinite(expiresIn) ||
    expiresIn <= 0
  )
    throw new Error("Couldn’t start subscription sign-in. Try again.");
  const parsed = new URL(url);
  if (
    parsed.protocol !== "https:" ||
    parsed.username ||
    parsed.password ||
    !(copilot
      ? parsed.hostname === "github.com"
      : ["auth.x.ai", "accounts.x.ai"].includes(parsed.hostname))
  )
    throw new Error("The provider returned an invalid sign-in URL.");
  return {
    kind: "device",
    url,
    userCode: json.user_code,
    deviceCode: json.device_code,
    interval: Math.max(1, interval) * 1000,
    expires: Date.now() + Math.min(900, expiresIn) * 1000,
    notBefore: Date.now() + Math.max(1, interval) * 1000,
  };
}

export async function completeClaudeConnect(
  session: Extract<SubscriptionSession, { kind: "code" }>,
  input: string,
  fetcher: typeof fetch,
  signal?: AbortSignal,
) {
  if (Date.now() >= session.expires)
    throw new Error("This sign-in expired. Start again.");
  let code = input.trim();
  let state: string | undefined;
  if (code.includes("://")) {
    try {
      const url = new URL(code);
      code = url.searchParams.get("code")?.trim() ?? "";
      state = url.searchParams.get("state") ?? undefined;
    } catch {
      throw new Error("Paste the authorization code from Claude.");
    }
  } else if (code.includes("#")) {
    [code, state] = code.split("#");
  }
  code = code.trim();
  state = state?.trim();
  if (!code || code.length > 8192 || /[\r\n]/.test(code))
    throw new Error("Paste the authorization code from Claude.");
  if (state && state !== session.state)
    throw new Error(
      "This authorization code belongs to another sign-in. Start again.",
    );
  const { status, json } = await post(
    CLAUDE_TOKEN_URL,
    {
      grant_type: "authorization_code",
      client_id: CLAUDE_CLIENT,
      code,
      state: state ?? session.state,
      redirect_uri: CLAUDE_REDIRECT,
      code_verifier: session.verifier,
    },
    fetcher,
    signal,
  );
  if (status >= 400)
    throw new Error("Couldn’t connect Claude. Start sign-in again.");
  return tokenCredential("claude", json);
}

async function copilotCredential(
  githubToken: string,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<SubscriptionCredential> {
  if (!/^[\x21-\x7e]+$/.test(githubToken))
    throw new Error("Invalid GitHub connection. Sign in again.");
  const { status, json } = await subscriptionJson(
    "https://api.github.com/copilot_internal/v2/token",
    {
      headers: {
        ...COPILOT_HEADERS,
        Accept: "application/json",
        Authorization: `Bearer ${githubToken}`,
      },
      signal,
    },
    fetcher,
  );
  const expires = Number(json.expires_at) * 1000;
  const value = parseSubscriptionCredential(
    "github_copilot",
    JSON.stringify({
      type: "oauth",
      access: json.token,
      refresh: githubToken,
      expires,
    }),
  );
  if (status >= 400 || !value || expires <= Date.now())
    throw new Error(
      "GitHub did not grant Copilot access. Check your subscription and sign in again.",
    );
  return value;
}

export async function pollSubscriptionConnect(
  provider: OAuthSubscriptionProvider,
  session: Extract<SubscriptionSession, { kind: "device" }>,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<SubscriptionCredential | null> {
  signal?.throwIfAborted();
  if (Date.now() >= session.expires)
    throw new Error("This sign-in code expired. Start again.");
  if (Date.now() < session.notBefore) return null;
  session.notBefore = Date.now() + session.interval;
  if (provider === "chatgpt")
    return pollChatgptDeviceCode(
      {
        deviceAuthId: session.deviceCode,
        userCode: session.userCode,
        expires: session.expires,
        interval: session.interval,
      },
      fetcher,
      signal,
    );
  if (provider === "claude")
    throw new Error("Claude requires an authorization code.");
  const copilot = provider === "github_copilot";
  const { status, json } = await post(
    copilot ? "https://github.com/login/oauth/access_token" : GROK_TOKEN_URL,
    {
      client_id: copilot ? COPILOT_CLIENT : GROK_CLIENT,
      device_code: session.deviceCode,
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
    },
    fetcher,
    signal,
    !copilot,
  );
  if (json.error === "slow_down") {
    session.interval += 5000;
    session.notBefore = Date.now() + session.interval;
    return null;
  }
  if (json.error === "authorization_pending") return null;
  if (
    status >= 400 ||
    json.error ||
    typeof json.access_token !== "string" ||
    !json.access_token
  )
    throw new Error(
      "Subscription authorization failed or was declined. Start again.",
    );
  return copilot
    ? copilotCredential(json.access_token, fetcher, signal)
    : tokenCredential(provider, json);
}

export async function refreshSubscriptionCredential(
  provider: OAuthSubscriptionProvider,
  previous: SubscriptionCredential,
  fetcher: typeof fetch,
) {
  if (provider === "chatgpt") {
    const parsed = parseChatgptCredential(JSON.stringify(previous));
    if (!parsed) throw new Error("Reconnect ChatGPT in Settings.");
    return refreshChatgptCredential(parsed, fetcher);
  }
  if (provider === "github_copilot")
    return copilotCredential(previous.refresh, fetcher);
  const { status, json } = await post(
    provider === "claude" ? CLAUDE_TOKEN_URL : GROK_TOKEN_URL,
    {
      grant_type: "refresh_token",
      client_id: provider === "claude" ? CLAUDE_CLIENT : GROK_CLIENT,
      refresh_token: previous.refresh,
    },
    fetcher,
    undefined,
    provider === "grok",
  );
  if (status >= 400)
    throw new Error("Reconnect your subscription in Settings.");
  return tokenCredential(provider, json, previous);
}

export async function listSubscriptionModels(
  provider: OAuthSubscriptionProvider,
  credential: SubscriptionCredential,
  fetcher: typeof fetch,
  signal?: AbortSignal,
): Promise<string[]> {
  if (provider === "chatgpt")
    return listChatgptModels(
      credential.access,
      credential.accountId,
      fetcher,
      signal,
    );
  const { status, json } = await subscriptionJson(
    `${defaultProviderConfig("llm", provider).baseUrl}/models`,
    {
      headers: {
        Authorization: `Bearer ${credential.access}`,
        ...(provider === "claude"
          ? CLAUDE_HEADERS
          : provider === "github_copilot"
            ? COPILOT_HEADERS
            : {}),
      },
      signal,
    },
    fetcher,
    8000,
  );
  if (status >= 400)
    throw new Error(
      status === 401 || status === 403
        ? "Reconnect your subscription in Settings."
        : "Couldn’t load subscription models. Try again.",
    );
  if (!Array.isArray(json.data))
    throw new Error("The provider returned an invalid model list.");
  const models = json.data.flatMap((entry) => {
    const model = record(entry);
    const capabilities = record(model.capabilities);
    if (
      model.model_picker_enabled === false ||
      capabilities.type === "embeddings" ||
      model.type === "embedding" ||
      (Array.isArray(model.supported_endpoints) &&
        !model.supported_endpoints.includes("/chat/completions")) ||
      (Array.isArray(model.endpoints) && !model.endpoints.includes("chat"))
    )
      return [];
    return typeof model.id === "string" &&
      model.id.trim() &&
      model.id.length <= 200 &&
      !/[\r\n]/.test(model.id)
      ? [model.id]
      : [];
  });
  return [...new Set(models)];
}
