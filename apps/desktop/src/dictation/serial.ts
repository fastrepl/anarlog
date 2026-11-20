let tail: Promise<void> = Promise.resolve();

/** Shortcut registration and teardown run in order, also across remounts. */
export function serialDictation(task: () => Promise<void>) {
  const next = tail.then(task);
  tail = next.catch(() => {});
  return next;
}

export function waitForDictationSetup() {
  return tail;
}
