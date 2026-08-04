export const MAX_QUEUED_CLARITY_OPERATIONS = 32;

export type ClarityFunction = ((...args: unknown[]) => void) & {
  q?: IArguments[];
  anarlogFallback?: true;
};

export function createClarityFallback(): ClarityFunction {
  const fallback: ClarityFunction = function clarity() {
    fallback.q = fallback.q ?? [];
    fallback.q.push(arguments);
    if (fallback.q.length > MAX_QUEUED_CLARITY_OPERATIONS) {
      fallback.q.splice(0, fallback.q.length - MAX_QUEUED_CLARITY_OPERATIONS);
    }
  };
  fallback.anarlogFallback = true;
  return fallback;
}

export function disableClarity(clarity: ClarityFunction | undefined) {
  if (!clarity) return;
  if (clarity.anarlogFallback) {
    clarity.q = [];
  }
  clarity("consentv2", {
    ad_Storage: "denied",
    analytics_Storage: "denied",
  });
  clarity("stop");
}
