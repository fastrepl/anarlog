const LOOPS_API_URL = "https://app.loops.so/api/v1";
const MAX_RATE_LIMIT_RETRIES = 3;

function getRetryDelayMs(response: Response) {
  const retryAfterSeconds = Number(response.headers.get("Retry-After") ?? "1");
  if (!Number.isFinite(retryAfterSeconds) || retryAfterSeconds < 0) {
    return 1_000;
  }
  return Math.min(retryAfterSeconds * 1_000, 5_000);
}

async function sendLoopsRequest({
  path,
  apiKey,
  idempotencyKey,
  body,
  fetcher,
}: {
  path: "/events/send" | "/transactional";
  apiKey: string;
  idempotencyKey: string;
  body: Record<string, unknown>;
  fetcher: typeof fetch;
}) {
  const request = {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      "Idempotency-Key": idempotencyKey,
    },
    body: JSON.stringify(body),
  } satisfies RequestInit;

  for (let attempt = 0; attempt <= MAX_RATE_LIMIT_RETRIES; attempt += 1) {
    const response = await fetcher(`${LOOPS_API_URL}${path}`, request);
    if (response.ok || response.status === 409) {
      return;
    }
    if (response.status === 429 && attempt < MAX_RATE_LIMIT_RETRIES) {
      await new Promise((resolve) =>
        setTimeout(resolve, getRetryDelayMs(response)),
      );
      continue;
    }
    throw new Error(
      `Loops request failed (${response.status}): ${await response.text()}`,
    );
  }
}

export function sendLoopsEvent({
  apiKey,
  email,
  userId,
  eventName,
  firstName,
  eventProperties,
  idempotencyKey,
  fetcher = fetch,
}: {
  apiKey: string;
  email: string;
  userId?: string;
  eventName: string;
  firstName?: string;
  eventProperties?: Record<string, string | number | boolean>;
  idempotencyKey: string;
  fetcher?: typeof fetch;
}) {
  return sendLoopsRequest({
    path: "/events/send",
    apiKey,
    idempotencyKey,
    body: {
      email,
      eventName,
      ...(userId ? { userId } : {}),
      ...(firstName ? { firstName } : {}),
      ...(eventProperties ? { eventProperties } : {}),
    },
    fetcher,
  });
}

export function sendLoopsTransactional({
  apiKey,
  transactionalId,
  email,
  dataVariables,
  idempotencyKey,
  fetcher = fetch,
}: {
  apiKey: string;
  transactionalId: string;
  email: string;
  dataVariables: Record<string, string | number>;
  idempotencyKey: string;
  fetcher?: typeof fetch;
}) {
  return sendLoopsRequest({
    path: "/transactional",
    apiKey,
    idempotencyKey,
    body: {
      transactionalId,
      email,
      dataVariables,
    },
    fetcher,
  });
}
