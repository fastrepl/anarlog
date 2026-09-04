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
  module?: string;
  stacktrace?: StacktraceLike;
  type?: string;
  value?: string;
};

type StacktraceLike = {
  frames?: Array<{
    colno?: number;
    filename?: string;
    function?: string;
    in_app?: boolean;
    lineno?: number;
    [key: string]: unknown;
  }>;
  [key: string]: unknown;
};

export type MobileErrorEvent = {
  breadcrumbs?: BreadcrumbLike[];
  exception?: { values?: ExceptionLike[] };
  extra?: Record<string, unknown>;
  contexts?: Record<string, unknown>;
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
const SAFE_STACK_FUNCTION_RE = /^[a-zA-Z0-9_$.[\]<>-]{1,128}$/;

function safeIdentifier(value: unknown): string | undefined {
  if (typeof value !== "string" || !SAFE_IDENTIFIER_RE.test(value)) {
    return undefined;
  }
  return value;
}

function safeStackFunction(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_STACK_FUNCTION_RE.test(value)
    ? value
    : undefined;
}

const SAFE_TAGS = new Set([
  "anarlog.error.stage",
  "anarlog.mobile.app_variant",
  "anarlog.mobile.execution_environment",
  "anarlog.mobile.os",
  "anarlog.operation",
  "anarlog.surface",
  "error.code",
  "error.type",
  "http.response.status_code",
  "service.name",
  "service.namespace",
]);

function sanitizeTags(tags: Record<string, unknown> | undefined) {
  if (!tags) return undefined;
  const safe = Object.fromEntries(
    Object.entries(tags).filter(
      ([key, value]) =>
        SAFE_TAGS.has(key) &&
        (typeof value === "number" || safeIdentifier(value) !== undefined),
    ),
  );
  return Object.keys(safe).length > 0 ? safe : undefined;
}

function sanitizeStacktrace(stacktrace: StacktraceLike): StacktraceLike {
  return {
    frames: stacktrace.frames?.map((frame) => ({
      colno: frame.colno,
      filename: "source",
      function: safeStackFunction(frame.function),
      in_app: frame.in_app,
      lineno: frame.lineno,
    })),
  };
}

export function sanitizeBreadcrumb<T extends BreadcrumbLike>(breadcrumb: T): T {
  return {
    category: breadcrumb.category,
    level: breadcrumb.level,
    ...(breadcrumb.category === "anarlog.operation"
      ? { message: safeIdentifier(breadcrumb.message) }
      : {}),
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
  } as T;
}

export function sanitizeMobileErrorEvent<T extends MobileErrorEvent>(event: T) {
  delete event.user;
  delete event.request;
  delete event.contexts;
  delete event.extra;
  event.tags = sanitizeTags(event.tags);
  event.breadcrumbs = event.breadcrumbs?.map(sanitizeBreadcrumb);

  const operation =
    typeof event.tags?.["anarlog.operation"] === "string"
      ? event.tags["anarlog.operation"]
      : undefined;
  const value = operation ? `${operation} failed` : "Mobile application error";
  for (const exception of event.exception?.values ?? []) {
    exception.value = value;
    delete exception.module;
    if (exception.stacktrace) {
      exception.stacktrace = sanitizeStacktrace(exception.stacktrace);
    }
    if (exception.mechanism) {
      const original = exception.mechanism;
      exception.mechanism = {
        type: safeIdentifier(original.type) ?? "generic",
        ...(original.handled === undefined
          ? {}
          : { handled: original.handled }),
        ...(original.synthetic === undefined
          ? {}
          : { synthetic: original.synthetic }),
      };
    }
  }
  if (event.message) event.message = value;
  delete event.logentry;

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

  return normalized;
}
