import { useEffect, useRef } from "react";

function isMacOS(userAgent: string) {
  const normalizedUserAgent = userAgent.toLowerCase();
  const isMobileAppleDevice = /iphone|ipad|ipod/.test(normalizedUserAgent);
  return (
    (normalizedUserAgent.includes("macintosh") ||
      normalizedUserAgent.includes("mac os x")) &&
    !isMobileAppleDevice
  );
}

export function openDeeplink(url: string) {
  if (isMacOS(window.navigator.userAgent)) {
    const iframe = document.createElement("iframe");
    iframe.src = url;
    iframe.setAttribute("aria-hidden", "true");
    iframe.tabIndex = -1;
    iframe.style.position = "absolute";
    iframe.style.width = "0";
    iframe.style.height = "0";
    iframe.style.border = "0";
    iframe.style.opacity = "0";
    iframe.style.pointerEvents = "none";
    document.body.append(iframe);

    return () => {
      iframe.remove();
    };
  }

  window.location.href = url;

  return undefined;
}

export function useAutoDeeplink(url: string | null) {
  const lastTriggeredUrl = useRef<string | null>(null);

  useEffect(() => {
    if (!url || lastTriggeredUrl.current === url) {
      return;
    }

    lastTriggeredUrl.current = url;

    return openDeeplink(url);
  }, [url]);
}
