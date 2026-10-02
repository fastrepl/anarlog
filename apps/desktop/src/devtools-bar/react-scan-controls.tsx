import { useRef, useState } from "react";

import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import {
  setReactInspecting,
  setReactOutlinesEnabled,
  setReactToolbarVisible,
  useReactToolsState,
} from "./react-tools";
import { setRenderOutlinesEnabled } from "./render-tracker";
import { ScanPanel } from "./scan-panel";

export function ReactScanControls() {
  const state = useReactToolsState();
  const [error, setError] = useState(false);
  const canceled = useRef(false);
  const dispose = useRef<(() => void) | undefined>(undefined);
  const pendingInstall = useRef<Promise<boolean> | null>(null);

  useMountEffect(() => {
    canceled.current = false;
    return () => {
      canceled.current = true;
      dispose.current?.();
    };
  });

  const install = () => {
    if (pendingInstall.current) return pendingInstall.current;

    pendingInstall.current = import("./react-scan")
      .then(({ installReactScan }) => {
        if (canceled.current) return false;
        dispose.current = installReactScan();
        setRenderOutlinesEnabled(false);
        return true;
      })
      .catch(() => {
        if (!canceled.current) setError(true);
        return false;
      });

    return pendingInstall.current;
  };

  const activate = (action: (enabled: boolean) => void, enabled: boolean) => {
    if (state.available) {
      action(!enabled);
      return;
    }
    if (pendingInstall.current) return;

    void install().then((installed) => {
      if (installed && !canceled.current) action(true);
    });
  };

  const buttonClass =
    "shrink-0 px-2 hover:bg-white/8 aria-pressed:bg-white/15 disabled:opacity-40";
  return (
    <>
      <button
        type="button"
        className={buttonClass}
        disabled={error}
        title={
          error
            ? "React Scan could not load. Reload to retry."
            : "Slowdown history, timings, and optimization prompts"
        }
        aria-label="Toggle React Scan panel"
        aria-pressed={state.toolbarVisible}
        onClick={() => activate(setReactToolbarVisible, state.toolbarVisible)}
      >
        SCAN
      </button>
      <button
        type="button"
        className={buttonClass}
        disabled={error}
        aria-label="Toggle React render outlines"
        aria-pressed={state.outlinesEnabled}
        onClick={() => activate(setReactOutlinesEnabled, state.outlinesEnabled)}
      >
        RENDERS
      </button>
      <button
        type="button"
        className={buttonClass}
        disabled={error}
        aria-label="Inspect React component"
        aria-pressed={state.inspecting}
        onClick={() => activate(setReactInspecting, state.inspecting)}
      >
        INSPECT
      </button>
      {state.toolbarVisible ? (
        <ScanPanel
          state={state}
          copyText={(text) => navigator.clipboard.writeText(text)}
        />
      ) : null}
    </>
  );
}
