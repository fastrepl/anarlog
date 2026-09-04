const SAFE_IDENTIFIER_RE = /^[a-zA-Z0-9_.:/-]{1,128}$/;
const SAFE_STACK_FUNCTION_RE = /^[a-zA-Z0-9_$.[\]<>-]{1,128}$/;
const SAFE_TAGS = new Set([
  "anarlog.error.stage",
  "anarlog.operation",
  "anarlog.surface",
  "error.code",
  "error.type",
  "http.response.status_code",
  "service.name",
  "service.namespace",
]);

function safeIdentifier(value) {
  return typeof value === "string" && SAFE_IDENTIFIER_RE.test(value)
    ? value
    : undefined;
}

function safeStackFunction(value) {
  return typeof value === "string" && SAFE_STACK_FUNCTION_RE.test(value)
    ? value
    : undefined;
}

export function sanitizeServerErrorEvent(event) {
  delete event.user;
  delete event.request;
  delete event.contexts;
  delete event.extra;
  delete event.message;
  delete event.logentry;
  delete event.transaction;
  event.tags = Object.fromEntries(
    Object.entries(event.tags ?? {}).filter(
      ([key, value]) =>
        SAFE_TAGS.has(key) &&
        (typeof value === "number" || safeIdentifier(value) !== undefined),
    ),
  );
  if (event.exception?.values) {
    event.exception.values = event.exception.values.map((exception) => {
      const sanitized = {
        ...exception,
        value: exception.type ? `${exception.type} captured` : "Error captured",
      };
      delete sanitized.module;
      if (sanitized.stacktrace) {
        sanitized.stacktrace.frames = sanitized.stacktrace.frames?.map(
          (frame) => ({
            colno: frame.colno,
            filename: "source",
            function: safeStackFunction(frame.function),
            in_app: frame.in_app,
            lineno: frame.lineno,
          }),
        );
      }
      if (sanitized.mechanism) {
        const original = sanitized.mechanism;
        sanitized.mechanism = {
          type: safeIdentifier(original.type) ?? "generic",
          ...(original.handled === undefined
            ? {}
            : { handled: original.handled }),
          ...(original.synthetic === undefined
            ? {}
            : { synthetic: original.synthetic }),
        };
      }
      return sanitized;
    });
  }
  event.breadcrumbs = event.breadcrumbs?.map((breadcrumb) => ({
    category: breadcrumb.category,
    level: breadcrumb.level,
    timestamp: breadcrumb.timestamp,
    type: breadcrumb.type,
  }));
  return event;
}
