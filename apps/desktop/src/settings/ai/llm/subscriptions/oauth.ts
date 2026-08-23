import {
  type OAuthCredential,
  parseOAuthCredential,
  serializeOAuthCredential,
} from "./credential";
import {
  getJson,
  numberField,
  oauthErrorMessage,
  postForm,
  postJson,
  stringField,
} from "./http";
import { createPkce, randomUrlToken } from "./pkce";

export const SUBSCRIPTION_PROVIDER_IDS = [
  "claude",
  "chatgpt",
  "grok",
  "github_copilot",
  "kimi_code",
] as const;

export type SubscriptionProviderId = (typeof SUBSCRIPTION_PROVIDER_IDS)[number];

export type CodeConnectSession = {
  kind: "code";
  url: string;
  verifier: string;
  state: string;
};

export type DeviceConnectSession = {
  kind: "device";
  userCode: string;
  verificationUrl: string;
  deviceCode: string;
  intervalMs: number;
};

export type ApiKeyConnectSession = {
  kind: "api_key";
  docsUrl: string;
};

export type ConnectSession =
  | CodeConnectSession
  | DeviceConnectSession
  | ApiKeyConnectSession;

const CLAUDE = {
  clientId: "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
  authorizeUrl: "https://claude.ai/oauth/authorize",
  tokenUrl: "https://platform.claude.com/v1/oauth/token",
  redirectUri: "https://platform.claude.com/oauth/code/callback",
  scope:
    "user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload",
} as const;

export const CHATGPT_CALLBACK_PORT = 1455;

const CHATGPT = {
  clientId: "app_EMoamEEZ73f0CkXaXp7hrann",
  authorizeUrl: "https://auth.openai.com/oauth/authorize",
  tokenUrl: "https://auth.openai.com/oauth/token",
  redirectUri: `http://localhost:${CHATGPT_CALLBACK_PORT}/auth/callback`,
  scope: "openid profile email offline_access",
} as const;

export const CHATGPT_API_BASE_URL = "https://chatgpt.com/backend-api/codex";

export const CHATGPT_REQUEST_HEADERS = {
  originator: "codex_cli_rs",
  "OpenAI-Beta": "responses=experimental",
  "User-Agent": "codex_cli_rs",
} as const;

const COPILOT = {
  clientId: "Iv1.b507a08c87ecfe98",
  deviceCodeUrl: "https://github.com/login/device/code",
  accessTokenUrl: "https://github.com/login/oauth/access_token",
  sessionTokenUrl: "https://api.github.com/copilot_internal/v2/token",
  userAgent: "GitHubCopilotChat/0.26.7",
  editorVersion: "vscode/1.99.3",
  pluginVersion: "copilot-chat/0.26.7",
} as const;

const GROK = {
  clientId: "b1a00492-073a-47ea-816f-4c329264a828",
  deviceCodeUrl: "https://auth.x.ai/oauth2/device/code",
  tokenUrl: "https://auth.x.ai/oauth2/token",
  scope: "openid profile email offline_access grok-cli:access api:access",
} as const;

export const KIMI_CODE_DOCS_URL =
  "https://www.kimi.com/en/help/kimi-code/membership-guide";

export const COPILOT_REQUEST_HEADERS = {
  "User-Agent": COPILOT.userAgent,
  "Editor-Version": COPILOT.editorVersion,
  "Editor-Plugin-Version": COPILOT.pluginVersion,
  "Copilot-Integration-Id": "vscode-chat",
} as const;

export const CLAUDE_OAUTH_HEADERS = {
  "anthropic-version": "2023-06-01",
  "anthropic-dangerous-direct-browser-access": "true",
  "anthropic-beta":
    "oauth-2025-04-20,interleaved-thinking-2025-05-14,context-management-2025-06-27,claude-code-20250219",
  "x-app": "cli",
  "user-agent": "claude-cli/2.1.81 (external, cli)",
} as const;

export function isSubscriptionProviderId(
  providerId: string,
): providerId is SubscriptionProviderId {
  return (SUBSCRIPTION_PROVIDER_IDS as readonly string[]).includes(providerId);
}

export function usesSubscriptionFetch(providerId: string, apiKey: string) {
  return (
    isSubscriptionProviderId(providerId) &&
    providerId !== "kimi_code" &&
    parseOAuthCredential(apiKey) !== null
  );
}

export function parseAuthorizationInput(input: string): {
  code: string;
  state?: string;
} {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("Paste the authorization code to continue.");
  }

  try {
    const url = new URL(trimmed);
    const code = url.searchParams.get("code")?.trim();
    if (code) {
      return {
        code,
        state: url.searchParams.get("state")?.trim() || undefined,
      };
    }
  } catch {
    // Not a URL — fall through to code#state parsing.
  }

  const hashIndex = trimmed.indexOf("#");
  if (hashIndex >= 0) {
    const code = trimmed.slice(0, hashIndex).trim();
    const state = trimmed.slice(hashIndex + 1).trim();
    if (!code) {
      throw new Error("Paste the authorization code to continue.");
    }
    return { code, state: state || undefined };
  }

  return { code: trimmed };
}

export function looksLikeAuthorizationInput(input: string): boolean {
  const trimmed = input.trim();
  if (!trimmed) {
    return false;
  }

  try {
    const parsed = parseAuthorizationInput(trimmed);
    if (trimmed.includes("://") && parsed.code) {
      return true;
    }
    if (trimmed.includes("#") && parsed.code && parsed.state) {
      return true;
    }
    return parsed.code.startsWith("ac_");
  } catch {
    return false;
  }
}

export function subscriptionAuthFromCallback(search: {
  access_token?: string | null;
  refresh_token?: string | null;
  code?: string | null;
  state?: string | null;
}): { code: string; state?: string } | null {
  const code = search.code?.trim();
  if (!code || search.access_token?.trim() || search.refresh_token?.trim()) {
    return null;
  }

  return {
    code,
    state: search.state?.trim() || undefined,
  };
}

export function authorizationInputFromParsed(parsed: {
  code: string;
  state?: string;
}) {
  return parsed.state ? `${parsed.code}#${parsed.state}` : parsed.code;
}

export function assertAuthorizationState(
  session: CodeConnectSession,
  parsed: { state?: string },
) {
  if (parsed.state && parsed.state !== session.state) {
    throw new Error("This sign-in expired. Try connecting again.");
  }
}

export async function startSubscriptionConnect(
  providerId: SubscriptionProviderId,
): Promise<ConnectSession> {
  switch (providerId) {
    case "claude":
      return startClaudeConnect();
    case "chatgpt":
      return startChatgptConnect();
    case "github_copilot":
      return startCopilotConnect();
    case "grok":
      return startGrokConnect();
    case "kimi_code":
      return { kind: "api_key", docsUrl: KIMI_CODE_DOCS_URL };
  }
}

export async function completeCodeConnect(
  providerId: Extract<SubscriptionProviderId, "claude" | "chatgpt">,
  session: CodeConnectSession,
  rawCode: string,
): Promise<string> {
  const parsed = parseAuthorizationInput(rawCode);
  assertAuthorizationState(session, parsed);
  if (providerId === "claude") {
    return serializeOAuthCredential(
      await exchangeClaudeCode({
        code: parsed.code,
        state: parsed.state ?? session.state,
        verifier: session.verifier,
      }),
    );
  }

  return serializeOAuthCredential(
    await exchangeChatgptCode({
      code: parsed.code,
      verifier: session.verifier,
    }),
  );
}

export async function pollDeviceConnect(
  providerId: Extract<SubscriptionProviderId, "github_copilot" | "grok">,
  session: DeviceConnectSession,
): Promise<"pending" | string> {
  if (providerId === "github_copilot") {
    const result = await pollCopilotDevice(session.deviceCode);
    return result === "pending" ? "pending" : serializeOAuthCredential(result);
  }

  const result = await pollGrokDevice(session.deviceCode);
  return result === "pending" ? "pending" : serializeOAuthCredential(result);
}

export async function refreshOAuthCredential(
  providerId: SubscriptionProviderId,
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  switch (providerId) {
    case "claude":
      return refreshClaude(credential);
    case "chatgpt":
      return refreshChatgpt(credential);
    case "github_copilot":
      return refreshCopilot(credential);
    case "grok":
      return refreshGrok(credential);
    case "kimi_code":
      throw new Error("Kimi Code uses an API key, not OAuth.");
  }
}

async function startClaudeConnect(): Promise<CodeConnectSession> {
  const pkce = await createPkce();
  const state = randomUrlToken(16);
  const url = new URL(CLAUDE.authorizeUrl);
  url.searchParams.set("code", "true");
  url.searchParams.set("client_id", CLAUDE.clientId);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("redirect_uri", CLAUDE.redirectUri);
  url.searchParams.set("scope", CLAUDE.scope);
  url.searchParams.set("code_challenge", pkce.challenge);
  url.searchParams.set("code_challenge_method", "S256");
  url.searchParams.set("state", state);
  return {
    kind: "code",
    url: url.toString(),
    verifier: pkce.verifier,
    state,
  };
}

async function startChatgptConnect(): Promise<CodeConnectSession> {
  const pkce = await createPkce();
  const state = randomUrlToken(16);
  const url = new URL(CHATGPT.authorizeUrl);
  url.searchParams.set("response_type", "code");
  url.searchParams.set("client_id", CHATGPT.clientId);
  url.searchParams.set("redirect_uri", CHATGPT.redirectUri);
  url.searchParams.set("scope", CHATGPT.scope);
  url.searchParams.set("code_challenge", pkce.challenge);
  url.searchParams.set("code_challenge_method", "S256");
  url.searchParams.set("state", state);
  url.searchParams.set("id_token_add_organizations", "true");
  url.searchParams.set("codex_cli_simplified_flow", "true");
  url.searchParams.set("originator", "codex_cli_rs");
  return {
    kind: "code",
    url: url.toString(),
    verifier: pkce.verifier,
    state,
  };
}

async function startCopilotConnect(): Promise<DeviceConnectSession> {
  const { status, json } = await postJson(
    COPILOT.deviceCodeUrl,
    {
      client_id: COPILOT.clientId,
      scope: "read:user",
    },
    { "User-Agent": COPILOT.userAgent },
  );
  const userCode = stringField(json, "user_code");
  const deviceCode = stringField(json, "device_code");
  const verificationUrl =
    stringField(json, "verification_uri_complete") ??
    stringField(json, "verification_uri");
  if (status >= 400 || !userCode || !deviceCode || !verificationUrl) {
    throw new Error(
      oauthErrorMessage(json, "Could not start GitHub Copilot login."),
    );
  }

  return {
    kind: "device",
    userCode,
    verificationUrl,
    deviceCode,
    intervalMs: (numberField(json, "interval") ?? 5) * 1000,
  };
}

async function startGrokConnect(): Promise<DeviceConnectSession> {
  const { status, json } = await postForm(GROK.deviceCodeUrl, {
    client_id: GROK.clientId,
    scope: GROK.scope,
  });
  const userCode = stringField(json, "user_code");
  const deviceCode = stringField(json, "device_code");
  const verificationUrl =
    stringField(json, "verification_uri_complete") ??
    stringField(json, "verification_uri");
  if (status >= 400 || !userCode || !deviceCode || !verificationUrl) {
    throw new Error(oauthErrorMessage(json, "Could not start Grok login."));
  }

  return {
    kind: "device",
    userCode,
    verificationUrl,
    deviceCode,
    intervalMs: (numberField(json, "interval") ?? 5) * 1000,
  };
}

async function exchangeClaudeCode(input: {
  code: string;
  state: string;
  verifier: string;
}): Promise<OAuthCredential> {
  const { status, json } = await postJson(CLAUDE.tokenUrl, {
    grant_type: "authorization_code",
    client_id: CLAUDE.clientId,
    code: input.code,
    state: input.state,
    redirect_uri: CLAUDE.redirectUri,
    code_verifier: input.verifier,
  });
  return credentialFromTokenResponse(
    json,
    status,
    "Could not connect Claude subscription.",
  );
}

async function exchangeChatgptCode(input: {
  code: string;
  verifier: string;
}): Promise<OAuthCredential> {
  const { status, json } = await postForm(CHATGPT.tokenUrl, {
    grant_type: "authorization_code",
    client_id: CHATGPT.clientId,
    code: input.code,
    redirect_uri: CHATGPT.redirectUri,
    code_verifier: input.verifier,
  });
  return credentialFromTokenResponse(
    json,
    status,
    "Could not connect ChatGPT subscription.",
  );
}

async function pollCopilotDevice(
  deviceCode: string,
): Promise<"pending" | OAuthCredential> {
  const { status, json } = await postJson(
    COPILOT.accessTokenUrl,
    {
      client_id: COPILOT.clientId,
      device_code: deviceCode,
      grant_type: "urn:ietf:params:oauth:grant-type:device_code",
    },
    { "User-Agent": COPILOT.userAgent },
  );
  const error = stringField(json, "error");
  if (error === "authorization_pending" || error === "slow_down") {
    return "pending";
  }
  const accessToken = stringField(json, "access_token");
  if (!accessToken) {
    if (status < 400 && !error) {
      return "pending";
    }
    throw new Error(
      oauthErrorMessage(json, "GitHub Copilot authorization failed."),
    );
  }

  return refreshCopilot({
    type: "oauth",
    refresh: accessToken,
    access: "",
    expires: 0,
  });
}

async function pollGrokDevice(
  deviceCode: string,
): Promise<"pending" | OAuthCredential> {
  const { status, json } = await postForm(GROK.tokenUrl, {
    client_id: GROK.clientId,
    device_code: deviceCode,
    grant_type: "urn:ietf:params:oauth:grant-type:device_code",
  });
  const error = stringField(json, "error");
  if (error === "authorization_pending" || error === "slow_down") {
    return "pending";
  }
  if (!stringField(json, "access_token")) {
    if (status < 400 && !error) {
      return "pending";
    }
    throw new Error(oauthErrorMessage(json, "Grok authorization failed."));
  }

  return credentialFromTokenResponse(
    json,
    status,
    "Grok authorization failed.",
  );
}

async function refreshClaude(
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  const { status, json } = await postJson(CLAUDE.tokenUrl, {
    grant_type: "refresh_token",
    client_id: CLAUDE.clientId,
    refresh_token: credential.refresh,
  });
  return credentialFromTokenResponse(
    json,
    status,
    "Could not refresh Claude subscription.",
    credential,
  );
}

async function refreshChatgpt(
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  const { status, json } = await postForm(CHATGPT.tokenUrl, {
    grant_type: "refresh_token",
    client_id: CHATGPT.clientId,
    refresh_token: credential.refresh,
  });
  return credentialFromTokenResponse(
    json,
    status,
    "Could not refresh ChatGPT subscription.",
    credential,
  );
}

async function refreshCopilot(
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  const { status, json } = await getJson(COPILOT.sessionTokenUrl, {
    Authorization: `Bearer ${credential.refresh}`,
    Accept: "application/json",
    ...COPILOT_REQUEST_HEADERS,
  });
  const token = stringField(json, "token");
  const expiresAt = numberField(json, "expires_at");
  if (status >= 400 || !token) {
    throw new Error(
      oauthErrorMessage(json, "Could not refresh GitHub Copilot access."),
    );
  }

  return {
    type: "oauth",
    refresh: credential.refresh,
    access: token,
    expires: expiresAt ? expiresAt * 1000 : Date.now() + 25 * 60 * 1000,
    accountId: credential.accountId,
  };
}

async function refreshGrok(
  credential: OAuthCredential,
): Promise<OAuthCredential> {
  const { status, json } = await postForm(GROK.tokenUrl, {
    grant_type: "refresh_token",
    client_id: GROK.clientId,
    refresh_token: credential.refresh,
  });
  return credentialFromTokenResponse(
    json,
    status,
    "Could not refresh Grok subscription.",
    credential,
  );
}

function credentialFromTokenResponse(
  json: Record<string, unknown>,
  status: number,
  fallback: string,
  previous?: OAuthCredential,
): OAuthCredential {
  const access = stringField(json, "access_token");
  if (status >= 400 || !access) {
    throw new Error(oauthErrorMessage(json, fallback));
  }

  const expiresIn = numberField(json, "expires_in") ?? 3600;
  return {
    type: "oauth",
    refresh: stringField(json, "refresh_token") ?? previous?.refresh ?? access,
    access,
    expires: Date.now() + expiresIn * 1000,
    accountId:
      parseChatgptAccountId(stringField(json, "id_token")) ??
      parseChatgptAccountId(access) ??
      stringField(json, "account_id") ??
      previous?.accountId,
  };
}

export function parseChatgptAccountId(jwt?: string): string | undefined {
  if (!jwt) {
    return undefined;
  }

  const payload = decodeJwtPayload(jwt);
  if (!payload) {
    return undefined;
  }

  const auth = payload["https://api.openai.com/auth"];
  if (auth && typeof auth === "object") {
    const nested = (auth as Record<string, unknown>).chatgpt_account_id;
    if (typeof nested === "string" && nested.length > 0) {
      return nested;
    }
  }

  const direct = payload.chatgpt_account_id;
  return typeof direct === "string" && direct.length > 0 ? direct : undefined;
}

function decodeJwtPayload(jwt: string): Record<string, unknown> | null {
  const payload = jwt.split(".")[1];
  if (!payload) {
    return null;
  }

  try {
    const padded =
      payload.replace(/-/g, "+").replace(/_/g, "/") +
      "=".repeat((4 - (payload.length % 4)) % 4);
    return JSON.parse(atob(padded)) as Record<string, unknown>;
  } catch {
    return null;
  }
}

export function chatgptCodexUrl(url: string): string {
  try {
    const parsed = new URL(url);
    if (parsed.hostname !== "api.openai.com") {
      return url;
    }
    const suffix = parsed.pathname.replace(/^\/v1(?=\/|$)/, "") || "/";
    return `${CHATGPT_API_BASE_URL}${suffix}${parsed.search}`;
  } catch {
    return url;
  }
}

export function chatgptResponsesBody(body: BodyInit | null | undefined) {
  if (typeof body !== "string") {
    return body;
  }

  try {
    const parsed = JSON.parse(body) as Record<string, unknown>;
    if (parsed.store === false) {
      return body;
    }
    return JSON.stringify({ ...parsed, store: false });
  } catch {
    return body;
  }
}

export function claudeMessagesUrl(url: string): string {
  try {
    const parsed = new URL(url);
    if (
      parsed.pathname.endsWith("/messages") &&
      !parsed.searchParams.has("beta")
    ) {
      parsed.searchParams.set("beta", "true");
      return parsed.toString();
    }
  } catch {
    return url;
  }
  return url;
}
