type BreadcrumbLike = {
  category?: string;
  data?: Record<string, unknown>;
  level?: string;
  message?: string;
  timestamp?: number;
  type?: string;
};

type ExceptionLike = {
  mechanism?: {
    data?: Record<string, unknown>;
    [key: string]: unknown;
  };
  type?: string;
  value?: string;
};

export type MobileErrorEvent = {
  breadcrumbs?: BreadcrumbLike[];
  exception?: { values?: ExceptionLike[] };
  extra?: Record<string, unknown>;
  logentry?: unknown;
  message?: string;
  platform?: string;
  request?: {
    method?: string;
    url?: string;
    [key: string]: unknown;
  };
  tags?: Record<string, unknown>;
  user?: {
    id?: string;
    [key: string]: unknown;
  };
};

type SafeErrorMetadata = {
  code?: string;
  stage?: string;
  status?: number;
  type: string;
};

const SAFE_IDENTIFIER_RE = /^[a-zA-Z0-9_.:/-]{1,128}$/;
const STACK_FRAME_RE = /^(?:\s*at\s|.*@.*:\d+:\d+$)/;

function safeIdentifier(value: unknown): string | undefined {
  if (typeof value !== "string" || !SAFE_IDENTIFIER_RE.test(value)) {
    return undefined;
  }
  return value;
}

function safeBreadcrumbData(
  data: Record<string, unknown> | undefined,
): Record<string, boolean | number | string> | undefined {
  const safe: Record<string, boolean | number | string> = {};
  for (const [key, value] of Object.entries(data ?? {})) {
    if (!safeIdentifier(key)) continue;
    if (typeof value === "boolean" || typeof value === "number") {
      safe[key] = value;
      continue;
    }
    const identifier = safeIdentifier(value);
    if (identifier !== undefined) {
      safe[key] = identifier;
    }
  }
  return Object.keys(safe).length > 0 ? safe : undefined;
}

export function sanitizeUrl(value: string | undefined) {
  if (!value) return value;

  try {
    const url = new URL(value);
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return value.split(/[?#]/, 1)[0];
  }
}

export function sanitizeBreadcrumb<T extends BreadcrumbLike>(breadcrumb: T): T {
  if (breadcrumb.category === "hyprnote.operation") {
    return {
      category: breadcrumb.category,
      level: breadcrumb.level,
      message: safeIdentifier(breadcrumb.message),
      timestamp: breadcrumb.timestamp,
      type: breadcrumb.type,
      data: safeBreadcrumbData(breadcrumb.data),
    } as T;
  }

  return {
    category: breadcrumb.category,
    level: breadcrumb.level,
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
    ...(breadcrumb.category === "navigation"
      ? {
          data: {
            from: sanitizeUrl(String(breadcrumb.data?.from ?? "")),
            to: sanitizeUrl(String(breadcrumb.data?.to ?? "")),
          },
        }
      : {}),
  } as T;
}

export function sanitizeMobileErrorEvent<T extends MobileErrorEvent>(event: T) {
  if (event.user) {
    event.user = event.user.id ? { id: event.user.id } : undefined;
  }

  if (event.request) {
    event.request = {
      method: event.request.method,
      url: sanitizeUrl(event.request.url),
    };
  }

  delete event.extra;
  event.breadcrumbs = event.breadcrumbs?.map(sanitizeBreadcrumb);

  const operation =
    typeof event.tags?.["hyprnote.operation"] === "string"
      ? event.tags["hyprnote.operation"]
      : undefined;
  if (event.platform === "javascript" || operation) {
    const value = operation
      ? `${operation} failed`
      : "Mobile application error";
    for (const exception of event.exception?.values ?? []) {
      exception.value = value;
      if (exception.mechanism) {
        delete exception.mechanism.data;
      }
    }
    if (event.message) {
      event.message = value;
    }
    delete event.logentry;
  }

  return event;
}

export function operationalErrorMetadata(error: unknown): SafeErrorMetadata {
  if (!error || typeof error !== "object") {
    return { type: "Error" };
  }

  const record = error as Record<string, unknown>;
  const statusValue = record.status ?? record.statusCode;
  const status =
    typeof statusValue === "number" &&
    Number.isInteger(statusValue) &&
    statusValue >= 100 &&
    statusValue <= 599
      ? statusValue
      : undefined;

  return {
    type: safeIdentifier(record.name) ?? "Error",
    code: safeIdentifier(record.code),
    stage: safeIdentifier(record.stage),
    status,
  };
}

export function normalizeOperationalError(
  error: unknown,
  operation: string,
): Error {
  const normalized = new Error(`${operation} failed`);
  const metadata = operationalErrorMetadata(error);
  normalized.name = metadata.type;

  if (error instanceof Error && error.stack) {
    const frames = error.stack
      .split("\n")
      .filter((line) => STACK_FRAME_RE.test(line));
    if (frames.length > 0) {
      normalized.stack = [
        `${normalized.name}: ${normalized.message}`,
        ...frames,
      ].join("\n");
    }
  }

  return normalized;
}
