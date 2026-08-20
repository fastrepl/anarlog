import { useLingui } from "@lingui/react/macro";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { type ReactNode, useEffect, useRef, useState } from "react";

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
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          if (useAppLock.getState().appUnlocked) return;
          if (useAppLock.getState().authenticating) return;
          promptedRef.current = true;
          void useAppLock.getState().unlockApp(DEVICE_AUTH_REASON.openApp);
          return;
        }
        if (useAppLock.getState().authenticating) return;
        promptedRef.current = false;
        lockApp();
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
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
