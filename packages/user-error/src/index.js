// Keep in sync with USER_ERROR_MARKERS in crates/user-error/src/lib.rs.
const USER_ERROR_MARKERS = [
  "billing_hard_limit_reached",
  "api key is invalid",
  "apikey is invalid",
  "credit balance is too low",
  "exceeded your current quota",
  "incorrect api key",
  "insufficient balance",
  "insufficient credits",
  "insufficient funds",
  "insufficient_quota",
  "invalid api key",
  "invalid apikey",
  "invalid x-api-key",
  "invalid_api_key",
  "not enough credits",
  "no quota",
  "payment required",
  "plans & billing",
  "plans and billing",
  "quota exceeded",
  "upgrade or purchase credits",
];

/** @param {string} value */
function textMatchesUserError(value) {
  const text = value.toLowerCase();
  return USER_ERROR_MARKERS.some((marker) => text.includes(marker));
}

/**
 * @param {unknown} value
 * @param {WeakSet<object>} seen
 * @returns {boolean}
 */
function valueMatchesUserError(value, seen) {
  if (typeof value === "string") return textMatchesUserError(value);
  if (typeof value !== "object" || value === null) return false;
  if (seen.has(value)) return false;
  seen.add(value);

  if (
    value instanceof Error &&
    textMatchesUserError(`${value.name}: ${value.message}`)
  ) {
    return true;
  }

  return Object.values(value).some((item) => valueMatchesUserError(item, seen));
}

/** @param {unknown} value */
export function isUserError(value) {
  return valueMatchesUserError(value, new WeakSet());
}

/** @param {unknown} event */
export function isUserErrorEvent(event) {
  if (typeof event !== "object" || event === null) return isUserError(event);

  const logentry = Reflect.get(event, "logentry");
  const exception = Reflect.get(event, "exception");
  const exceptionValues =
    typeof exception === "object" && exception !== null
      ? Reflect.get(exception, "values")
      : undefined;
  const tags = Reflect.get(event, "tags");
  const contexts = Reflect.get(event, "contexts");

  return (
    [
      Reflect.get(event, "message"),
      typeof logentry === "object" && logentry !== null
        ? Reflect.get(logentry, "message")
        : undefined,
      typeof logentry === "object" && logentry !== null
        ? Reflect.get(logentry, "params")
        : undefined,
      Reflect.get(event, "extra"),
      typeof tags === "object" && tags !== null
        ? Object.values(tags)
        : undefined,
      typeof contexts === "object" && contexts !== null
        ? Reflect.get(contexts, "anarlog.operation")
        : undefined,
    ].some(isUserError) ||
    (Array.isArray(exceptionValues) &&
      exceptionValues.some(
        (value) =>
          typeof value === "object" &&
          value !== null &&
          [Reflect.get(value, "type"), Reflect.get(value, "value")].some(
            isUserError,
          ),
      ))
  );
}
