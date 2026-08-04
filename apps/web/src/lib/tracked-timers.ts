type TimerHandle = ReturnType<typeof setTimeout>;

export function createTrackedTimers() {
  const timeouts = new Set<TimerHandle>();
  const intervals = new Set<TimerHandle>();

  return {
    setTimeout(callback: () => void, delay: number) {
      const handle = setTimeout(() => {
        timeouts.delete(handle);
        callback();
      }, delay);
      timeouts.add(handle);
      return handle;
    },
    setInterval(callback: () => void, delay: number) {
      const handle = setInterval(callback, delay);
      intervals.add(handle);
      return handle;
    },
    clearInterval(handle: TimerHandle) {
      clearInterval(handle);
      intervals.delete(handle);
    },
    clear() {
      timeouts.forEach(clearTimeout);
      intervals.forEach(clearInterval);
      timeouts.clear();
      intervals.clear();
    },
    pendingCount() {
      return timeouts.size + intervals.size;
    },
  };
}
