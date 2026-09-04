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

  return ["message", "logentry", "exception", "extra", "tags", "contexts"].some(
    (key) => isUserError(Reflect.get(event, key)),
  );
}
