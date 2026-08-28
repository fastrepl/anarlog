import * as stylex from "@stylexjs/stylex";
import { useEffect, useState } from "react";

import {
  commands as windowsCommands,
  events as windowsEvents,
  type FloatingBarState,
} from "@anlg/plugin-windows";

import { FloatingBarOverlay } from "./bar";

export function FloatingBarOverlayScreen() {
  const [state, setState] = useState<FloatingBarState | null>(null);

  useEffect(() => {
    document.documentElement.dataset.floatingBar = "";

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    void windowsCommands.floatingBarCurrentState().then((result) => {
      if (cancelled || result.status === "error" || !result.data) {
        return;
      }
      setState(result.data);
    });

    windowsEvents.floatingBarOverlayState
      .listen((event) => {
        if (!cancelled) {
          setState(event.payload.state);
        }
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }
        unlisteners.push(unlisten);
      });

    windowsEvents.floatingBarOverlayAmplitude
      .listen((event) => {
        if (cancelled) {
          return;
        }
        setState((current) =>
          current
            ? { ...current, amplitude: event.payload.amplitude }
            : current,
        );
      })
      .then((unlisten) => {
        if (cancelled) {
          unlisten();
          return;
        }
        unlisteners.push(unlisten);
      });

    return () => {
      cancelled = true;
      delete document.documentElement.dataset.floatingBar;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  if (!state) {
    return <div {...stylex.props(styles.root)} />;
  }

  return (
    <div {...stylex.props(styles.root)}>
      <FloatingBarOverlay
        state={state}
        onStop={() => {
          void windowsEvents.floatingBarStop.emit({});
        }}
        onToggleExpanded={(expanded) => {
          void windowsEvents.floatingBarSettingsChange.emit({
            floatingBarOpacity: null,
            liveCaptionOpacity: null,
            liveCaptionWidth: null,
            liveCaptionLineCount: null,
            liveCaptionPosition: null,
            liveCaptionMinimized: !expanded,
          });
        }}
      />
    </div>
  );
}

const styles = stylex.create({
  root: {
    backgroundColor: "transparent",
    height: "100vh",
    width: "100vw",
  },
});
