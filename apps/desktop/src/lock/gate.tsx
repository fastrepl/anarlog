import { useLingui } from "@lingui/react/macro";
import { type ReactNode, useEffect, useRef, useState } from "react";

import {
  events as windowsEvents,
  getCurrentWebviewWindowLabel,
} from "@anlg/plugin-windows";

import { DEVICE_AUTH_REASON } from "./auth";
import { LockScreen, useDeviceAuthHint } from "./screen";
import { useAppLock } from "./store";

import { useSettingsReady } from "~/settings/queries";
import { BrandLoadingView } from "~/shared/brand-loading-view";
import { useConfigValue } from "~/shared/config";

export function AppLockGate({ children }: { children: ReactNode }) {
  const { t } = useLingui();
  const settingsReady = useSettingsReady();
  const lockAppEnabled = useConfigValue("lock_app");
  const available = useAppLock((state) => state.available);
  const appUnlocked = useAppLock((state) => state.appUnlocked);
  const authenticating = useAppLock((state) => state.authenticating);
  const refreshAvailability = useAppLock((state) => state.refreshAvailability);
  const unlockApp = useAppLock((state) => state.unlockApp);
  const lockApp = useAppLock((state) => state.lockApp);
  const hint = useDeviceAuthHint();
  const promptedRef = useRef(false);
  const [sessionStarted, setSessionStarted] = useState(false);

  useEffect(() => {
    void refreshAvailability();
  }, [refreshAvailability]);

  const shouldLock = settingsReady && lockAppEnabled && available === true;
  const locked = shouldLock && !appUnlocked;

  useEffect(() => {
    if (!shouldLock || appUnlocked) {
      setSessionStarted(true);
    }
  }, [appUnlocked, shouldLock]);

  useEffect(() => {
    if (!shouldLock || appUnlocked || authenticating || promptedRef.current) {
      return;
    }
    promptedRef.current = true;
    void unlockApp(DEVICE_AUTH_REASON.openApp);
  }, [unlockApp, appUnlocked, authenticating, shouldLock]);

  useEffect(() => {
    if (!shouldLock) return;

    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void windowsEvents.visibilityEvent
      .listen(({ payload }) => {
        if (
          payload.window.type !== "main" ||
          getCurrentWebviewWindowLabel() !== "main"
        ) {
          return;
        }

        if (!payload.visible) {
          // Closing hides the main window. Lock now, but do not prompt until
          // the user opens it again.
          promptedRef.current = true;
          lockApp();
          return;
        }

        if (useAppLock.getState().appUnlocked) return;
        if (useAppLock.getState().authenticating) {
          // A close-invalidated prompt is still running. Let the auto-prompt
          // effect start a new one once that auth settles.
          promptedRef.current = false;
          return;
        }
        promptedRef.current = true;
        void useAppLock.getState().unlockApp(DEVICE_AUTH_REASON.openApp);
      })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [lockApp, shouldLock]);

  if (!settingsReady) {
    return <BrandLoadingView />;
  }

  if (lockAppEnabled && available === null) {
    return <BrandLoadingView />;
  }

  return (
    <div className="relative h-full min-h-0 w-full">
      {sessionStarted ? (
        <div
          className="h-full min-h-0 w-full"
          {...(locked ? { inert: true } : {})}
        >
          {children}
        </div>
      ) : null}
      {locked ? (
        <div className="absolute inset-0">
          <LockScreen
            title={t`Anarlog is Locked`}
            description={hint}
            action={t`View Anarlog`}
            authenticating={authenticating}
            onUnlock={() => {
              promptedRef.current = true;
              void unlockApp(DEVICE_AUTH_REASON.openApp);
            }}
          />
        </div>
      ) : null}
    </div>
  );
}
