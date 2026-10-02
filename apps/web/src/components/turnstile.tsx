import { useRef, useState } from "react";

import { env } from "@/env";
import { useMountEffect } from "@/hooks/useMountEffect";

type TurnstileApi = {
  render: (
    container: HTMLElement,
    options: {
      sitekey: string;
      callback: (token: string) => void;
      "expired-callback": () => void;
      "error-callback": () => void;
    },
  ) => string;
  remove: (widgetId: string) => void;
};

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const turnstileScriptUrl =
  "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

let turnstileScriptPromise: Promise<TurnstileApi> | undefined;

export const isTurnstileEnabled = Boolean(env.VITE_TURNSTILE_SITE_KEY);

function loadTurnstile(): Promise<TurnstileApi> {
  if (window.turnstile) {
    return Promise.resolve(window.turnstile);
  }
  if (turnstileScriptPromise) {
    return turnstileScriptPromise;
  }

  const scriptPromise = new Promise<TurnstileApi>((resolve, reject) => {
    const script = document.createElement("script");
    script.async = true;
    script.src = turnstileScriptUrl;
    script.addEventListener(
      "load",
      () => {
        if (window.turnstile) {
          resolve(window.turnstile);
        } else {
          reject(new Error("Turnstile API did not initialize"));
        }
      },
      { once: true },
    );
    script.addEventListener(
      "error",
      () => reject(new Error("Turnstile API failed to load")),
      { once: true },
    );
    document.head.append(script);
  }).catch((error: unknown) => {
    turnstileScriptPromise = undefined;
    throw error;
  });
  turnstileScriptPromise = scriptPromise;

  return scriptPromise;
}

export function Turnstile({
  onToken,
}: {
  onToken: (token: string | undefined) => void;
}) {
  const siteKey = env.VITE_TURNSTILE_SITE_KEY;
  const [retryKey, setRetryKey] = useState(0);
  const [hasError, setHasError] = useState(false);
  if (!siteKey) {
    return null;
  }

  if (hasError) {
    return (
      <div className="flex flex-col items-center gap-1 text-center">
        <p className="text-color-muted text-sm">
          Couldn't load the verification check.
        </p>
        <button
          type="button"
          onClick={() => {
            onToken(undefined);
            setHasError(false);
            setRetryKey((key) => key + 1);
          }}
          className="text-color-muted hover:text-color cursor-pointer text-sm underline transition-colors"
        >
          Retry
        </button>
      </div>
    );
  }

  return (
    <TurnstileWidget
      key={retryKey}
      siteKey={siteKey}
      onToken={onToken}
      onError={() => setHasError(true)}
    />
  );
}

function TurnstileWidget({
  siteKey,
  onToken,
  onError,
}: {
  siteKey: string;
  onToken: (token: string | undefined) => void;
  onError: () => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const onTokenRef = useRef(onToken);
  const onErrorRef = useRef(onError);
  onTokenRef.current = onToken;
  onErrorRef.current = onError;

  useMountEffect(() => {
    let disposed = false;
    let widgetId: string | undefined;
    let turnstile: TurnstileApi | undefined;
    const fail = () => {
      onTokenRef.current(undefined);
      onErrorRef.current();
    };

    void loadTurnstile()
      .then((api) => {
        if (disposed || !containerRef.current) {
          return;
        }

        turnstile = api;
        widgetId = api.render(containerRef.current, {
          sitekey: siteKey,
          callback: (token) => onTokenRef.current(token),
          "expired-callback": () => onTokenRef.current(undefined),
          "error-callback": () => {
            if (!disposed) {
              fail();
            }
          },
        });
      })
      .catch(() => {
        if (!disposed) {
          fail();
        }
      });

    return () => {
      disposed = true;
      if (widgetId !== undefined) {
        turnstile?.remove(widgetId);
      }
    };
  });

  return <div ref={containerRef} />;
}
