import { useEffect, useState } from "react";

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
  useEffect(() => {
    let canceled = false;
    let dispose: (() => void) | undefined;
    // The bar is mounted only after the native showDevtool gate succeeds.
    // The existing render-hook is already installed before react-dom loads.
    void import("./react-scan")
      .then(({ installReactScan }) => {
        if (canceled) return;
        dispose = installReactScan();
        setRenderOutlinesEnabled(false);
      })
      .catch(() => {
        if (!canceled) setError(true);
      });
    return () => {
      canceled = true;
      dispose?.();
    };
  }, []);

  const buttonClass =
    "shrink-0 px-2 hover:bg-white/8 aria-pressed:bg-white/15 disabled:opacity-40";
  return (
    <>
      <button
        type="button"
        className={buttonClass}
        disabled={!state.available}
        title={
          error
            ? "React Scan could not load. Reload to retry."
            : "Slowdown history, timings, and optimization prompts"
        }
        aria-label="Toggle React Scan panel"
        aria-pressed={state.toolbarVisible}
        onClick={() => setReactToolbarVisible(!state.toolbarVisible)}
      >
        SCAN
      </button>
      <button
        type="button"
        className={buttonClass}
        disabled={!state.available}
        aria-label="Toggle React render outlines"
        aria-pressed={state.outlinesEnabled}
        onClick={() => setReactOutlinesEnabled(!state.outlinesEnabled)}
      >
        RENDERS
      </button>
      <button
        type="button"
        className={buttonClass}
        disabled={!state.available}
        aria-label="Inspect React component"
        aria-pressed={state.inspecting}
        onClick={() => setReactInspecting(!state.inspecting)}
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
