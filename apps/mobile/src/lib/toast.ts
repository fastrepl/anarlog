export type Toast = {
  id: string;
  title: string;
  description?: string;
};

const TOAST_DURATION_MS = 5_000;

let current: Toast | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();

function publish(next: Toast | null) {
  current = next;
  listeners.forEach((listener) => listener());
}

export function getToast(): Toast | null {
  return current;
}

export function subscribeToast(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function showToast(toast: Toast) {
  clearTimeout(timer);
  publish(toast);
  timer = setTimeout(() => dismissToast(toast.id), TOAST_DURATION_MS);
}

export function dismissToast(id?: string) {
  if (!current || (id !== undefined && current.id !== id)) return;
  clearTimeout(timer);
  timer = undefined;
  publish(null);
}
