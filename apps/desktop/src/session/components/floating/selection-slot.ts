let host: HTMLElement | null = null;
const listeners = new Set<() => void>();

export function setSessionFabSelectionHost(node: HTMLElement | null) {
  if (host === node) {
    return;
  }

  host = node;
  for (const listener of listeners) {
    listener();
  }
}

export function subscribeSessionFabSelectionHost(onStoreChange: () => void) {
  listeners.add(onStoreChange);
  return () => {
    listeners.delete(onStoreChange);
  };
}

export function getSessionFabSelectionHost() {
  return host;
}
