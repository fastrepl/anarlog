const trialStarts = new Map<string, Promise<unknown>>();

export function startTrialOnce<T>(
  userId: string,
  start: () => Promise<T>,
): Promise<T> {
  const existing = trialStarts.get(userId);
  if (existing) {
    return existing as Promise<T>;
  }

  const request = start().catch((error) => {
    trialStarts.delete(userId);
    throw error;
  });
  trialStarts.set(userId, request);
  return request;
}
