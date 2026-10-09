export function reportDownloadConversion(
  gtag: ((...args: unknown[]) => void) | undefined,
  onComplete?: () => void,
) {
  let completed = false;
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const finish = () => {
    if (completed) return;
    completed = true;
    clearTimeout(timeout);
    onComplete?.();
  };
  if (!gtag) {
    finish();
    return;
  }
  if (onComplete) timeout = setTimeout(finish, 1000);
  try {
    gtag("event", "conversion", {
      send_to: "AW-18481972229/i7OTCMKHiJUdEIWI8uxE",
      event_callback: finish,
      event_timeout: 1000,
    });
  } catch {
    finish();
  }
}
