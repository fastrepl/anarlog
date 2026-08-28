import * as stylex from "@stylexjs/stylex";
import { useEffect, useState } from "react";

import {
  commands as windowsCommands,
  events as windowsEvents,
  type LiveCaptionState,
} from "@anlg/plugin-windows";
import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

const LIVE_CAPTION_MIN_WIDTH = 260;
const LIVE_CAPTION_MAX_WIDTH = 640;
const LIVE_CAPTION_MIN_LINE_COUNT = 1;
const LIVE_CAPTION_MAX_LINE_COUNT = 4;
const LIVE_CAPTION_LINE_HEIGHT = 22;
const LIVE_CAPTION_HORIZONTAL_PADDING = 16;
const LIVE_CAPTION_VERTICAL_PADDING = 10;
const LIVE_CAPTION_FOOTER_HEIGHT = 32;
const LIVE_CAPTION_FOOTER_SEPARATOR_HEIGHT = 1;
const LIVE_CAPTION_CORNER_RADIUS = 12;
const LIVE_CAPTION_MIN_OPACITY = 0.05;
const LIVE_CAPTION_MAX_OPACITY = 1;

export function LiveCaptionOverlayScreen() {
  const [state, setState] = useState<LiveCaptionState | null>(null);

  useEffect(() => {
    document.documentElement.dataset.liveCaption = "";

    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    void windowsCommands.liveCaptionCurrentState().then((result) => {
      if (cancelled || result.status === "error" || !result.data) {
        return;
      }
      setState(result.data);
    });

    windowsEvents.liveCaptionOverlayState
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

    return () => {
      cancelled = true;
      delete document.documentElement.dataset.liveCaption;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  if (!state || state.minimized) {
    return <div {...stylex.props(styles.screen)} />;
  }

  return (
    <div {...stylex.props(styles.screen)}>
      <LiveCaptionOverlay
        state={state}
        onOpacityChange={(opacity) => {
          void windowsEvents.floatingBarSettingsChange.emit({
            floatingBarOpacity: null,
            liveCaptionOpacity: opacity,
            liveCaptionWidth: null,
            liveCaptionLineCount: null,
            liveCaptionPosition: null,
            liveCaptionMinimized: null,
          });
        }}
        onHide={() => {
          void windowsEvents.floatingBarSettingsChange.emit({
            floatingBarOpacity: null,
            liveCaptionOpacity: null,
            liveCaptionWidth: null,
            liveCaptionLineCount: null,
            liveCaptionPosition: null,
            liveCaptionMinimized: true,
          });
        }}
      />
    </div>
  );
}

export function LiveCaptionOverlay({
  state,
  onOpacityChange,
  onHide,
}: {
  state: LiveCaptionState;
  onOpacityChange: (opacity: number) => void;
  onHide: () => void;
}) {
  const lineCount = Math.min(
    Math.max(state.lineCount, LIVE_CAPTION_MIN_LINE_COUNT),
    LIVE_CAPTION_MAX_LINE_COUNT,
  );
  const width = Math.min(
    Math.max(state.width, LIVE_CAPTION_MIN_WIDTH),
    LIVE_CAPTION_MAX_WIDTH,
  );
  const opacity = Math.min(
    Math.max(state.opacity, LIVE_CAPTION_MIN_OPACITY),
    LIVE_CAPTION_MAX_OPACITY,
  );

  return (
    <div data-tauri-drag-region {...stylex.props(styles.root)}>
      <div
        {...mergeStyleXProps(styles.panel, undefined, {
          width,
          borderRadius: LIVE_CAPTION_CORNER_RADIUS,
          background: `rgba(0, 0, 0, ${opacity})`,
        })}
      >
        <p
          data-tauri-drag-region
          {...mergeStyleXProps(styles.caption, undefined, {
            padding: `${LIVE_CAPTION_VERTICAL_PADDING}px ${LIVE_CAPTION_HORIZONTAL_PADDING}px`,
            minHeight:
              LIVE_CAPTION_LINE_HEIGHT * lineCount +
              LIVE_CAPTION_VERTICAL_PADDING * 2,
            maxHeight:
              LIVE_CAPTION_LINE_HEIGHT * lineCount +
              LIVE_CAPTION_VERTICAL_PADDING * 2,
            display: "-webkit-box",
            WebkitBoxOrient: "vertical",
            WebkitLineClamp: lineCount,
            overflow: "hidden",
          })}
        >
          {state.text}
        </p>
        <div
          {...mergeStyleXProps(styles.separator, undefined, {
            height: LIVE_CAPTION_FOOTER_SEPARATOR_HEIGHT,
            background: "rgba(255, 255, 255, 0.16)",
          })}
        />
        <div
          {...mergeStyleXProps(styles.footer, undefined, {
            height: LIVE_CAPTION_FOOTER_HEIGHT,
            paddingLeft: LIVE_CAPTION_HORIZONTAL_PADDING,
            paddingRight: 8,
            gap: 8,
          })}
        >
          <input
            type="range"
            min={LIVE_CAPTION_MIN_OPACITY}
            max={LIVE_CAPTION_MAX_OPACITY}
            step={0.01}
            value={opacity}
            aria-label="Transcript opacity"
            data-tauri-drag-region="false"
            onChange={(event) => {
              onOpacityChange(Number(event.currentTarget.value));
            }}
            {...stylex.props(styles.opacitySlider)}
          />
          <span {...stylex.props(styles.spacer)} />
          <button
            type="button"
            data-tauri-drag-region="false"
            aria-label="Hide transcript"
            onClick={onHide}
            {...mergeStyleXProps(styles.hideButton, undefined, {
              background: "rgba(0, 0, 0, 0.42)",
              boxShadow: "inset 0 0 0 0.5px rgba(255, 255, 255, 0.18)",
            })}
          >
            Hide
          </button>
        </div>
      </div>
    </div>
  );
}

const styles = stylex.create({
  caption: {
    color: "white",
    fontSize: "16px",
    fontWeight: 500,
    lineHeight: "22px",
    margin: 0,
    textAlign: "center",
  },
  footer: {
    alignItems: "center",
    display: "flex",
  },
  hideButton: {
    borderRadius: "9999px",
    color: "rgb(255 255 255 / 0.9)",
    fontSize: "11px",
    fontWeight: 600,
    height: "1.25rem",
    paddingInline: "0.5rem",
  },
  opacitySlider: {
    appearance: "none",
    backgroundColor: "rgb(255 255 255 / 0.25)",
    borderRadius: "9999px",
    cursor: "pointer",
    height: "0.25rem",
    width: "120px",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  root: {
    alignItems: "stretch",
    display: "flex",
    height: "100%",
    justifyContent: "center",
    width: "100%",
  },
  screen: {
    backgroundColor: "transparent",
    height: "100vh",
    width: "100vw",
  },
  separator: {
    flexShrink: 0,
  },
  spacer: {
    flex: "1",
  },
});
