import { useQuery } from "@tanstack/react-query";
import { getIdentifier, getVersion } from "@tauri-apps/api/app";
import { useReducer, useSyncExternalStore } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";
import { commands as windowsCommands } from "@anlg/plugin-windows";
import { cn } from "@anlg/utils";

import {
  formatBytes,
  HISTORY_LENGTH,
  startDevtoolsMetrics,
  useDevtoolsMetrics,
} from "./metrics";
import {
  areReactScanOutlinesEnabled,
  ignoreReactScan,
  isReactScanAvailable,
  isReactScanToolbarVisible,
  setReactScanOutlinesEnabled,
  setReactScanToolbarVisible,
  subscribeReactScanAvailability,
} from "./react-scan";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { commands } from "~/types/tauri.gen";

export type BuildChannel = "dev" | "staging" | "stable";

export function resolveBuildChannel(identifier: string): BuildChannel {
  if (identifier.endsWith(".staging")) return "staging";
  if (identifier.endsWith(".dev")) return "dev";
  return "stable";
}

const CHANNEL_CLASSES: Record<BuildChannel, string> = {
  dev: "bg-blue-900 text-blue-50",
  staging: "bg-amber-900 text-amber-50",
  stable: "bg-neutral-800 text-neutral-100",
};

const LABEL_CLASS = "uppercase opacity-60";
const VALUE_CLASS = "tabular-nums";

/**
 * VS Code-style status bar for dev and staging builds. Stable builds never
 * render it (`show_devtool` is false there), so copy stays English-only.
 */
export function DevtoolsStatusBar(props: Record<never, never>) {
  ignoreReactScan(props);

  const enabledQuery = useQuery({
    queryKey: ["devtools-panel", "enabled"],
    queryFn: commands.showDevtool,
    staleTime: Infinity,
  });

  if (enabledQuery.data !== true) {
    return null;
  }

  return <DevtoolsStatusBarContent />;
}

function DevtoolsStatusBarContent(props: Record<never, never>) {
  ignoreReactScan(props);
  useMountEffect(() => startDevtoolsMetrics());

  const metrics = useDevtoolsMetrics();
  const build = useBuildInfo();
  const reactScanAvailable = useSyncExternalStore(
    subscribeReactScanAvailability,
    isReactScanAvailable,
  );
  const [, refresh] = useReducer((tick: number) => tick + 1, 0);

  const fps = last(metrics.fps);
  const invokes = last(metrics.invokes) ?? 0;
  const callbacks = last(metrics.callbacks) ?? 0;
  const renders = last(metrics.renders) ?? 0;
  const memoryBytes = last(metrics.memoryBytes);
  const outlinesEnabled = reactScanAvailable && areReactScanOutlinesEnabled();
  const toolbarVisible = reactScanAvailable && isReactScanToolbarVisible();

  if (!build) {
    return null;
  }

  const channel = build.channel;

  return (
    <footer
      aria-label="Developer status bar"
      data-testid="devtools-status-bar"
      className={cn([
        "flex h-6 shrink-0 items-stretch overflow-hidden select-none",
        "font-mono text-[11px] leading-none",
        CHANNEL_CLASSES[channel],
      ])}
    >
      <BarButton
        title="Open Devtools panel"
        onClick={() => void openDevtoolsPanel()}
      >
        <span className="font-semibold tracking-wider uppercase">
          {channel}
        </span>
        <span className="opacity-70">
          {build.version}
          {build.hash ? ` ${build.hash}` : ""}
        </span>
      </BarButton>

      <Segment
        label="FPS"
        title={`Frames per second (avg ${average(metrics.fps)} over ${metrics.fps.length}s)`}
      >
        <span className={VALUE_CLASS}>{fps ?? "–"}</span>
        <Sparkline values={metrics.fps} floor={60} />
      </Segment>

      <Segment
        label="IPC"
        title="Tauri IPC per second: ↑ invokes, ↓ callbacks (responses, events, channels)"
      >
        <span className={VALUE_CLASS}>
          ↑{invokes} ↓{callbacks}
        </span>
        <Sparkline
          values={metrics.invokes.map(
            (value, index) => value + (metrics.callbacks[index] ?? 0),
          )}
        />
      </Segment>

      {reactScanAvailable ? (
        <BarButton
          title={
            outlinesEnabled
              ? "React Scan: outlining re-renders (click to pause)"
              : "React Scan: outlines paused (click to resume)"
          }
          onClick={() => {
            setReactScanOutlinesEnabled(!outlinesEnabled);
            refresh();
          }}
        >
          <span className={LABEL_CLASS}>renders</span>
          <span className={VALUE_CLASS}>
            {outlinesEnabled ? renders : "off"}
          </span>
          <Sparkline values={metrics.renders} />
        </BarButton>
      ) : null}

      <Segment
        label="MEM"
        title="Host process resident memory (WebKit content process is separate on macOS)"
      >
        <span className={VALUE_CLASS}>
          {memoryBytes === undefined ? "–" : formatBytes(memoryBytes)}
        </span>
        <Sparkline values={metrics.memoryBytes} relative />
      </Segment>

      <div className="flex-1" />

      {reactScanAvailable ? (
        <BarButton
          title="Toggle the React Scan toolbar (inspector, slowdown notifications)"
          onClick={() => {
            setReactScanToolbarVisible(!toolbarVisible);
            refresh();
          }}
        >
          <span className={cn([VALUE_CLASS, !toolbarVisible && "opacity-70"])}>
            {toolbarVisible ? "◉" : "○"} react scan
          </span>
        </BarButton>
      ) : null}
    </footer>
  );
}

function useBuildInfo() {
  return useQuery({
    queryKey: ["devtools-bar", "build"],
    staleTime: Infinity,
    queryFn: async () => {
      const [identifier, version, hash] = await Promise.all([
        getIdentifier(),
        getVersion(),
        miscCommands.getGitHash(),
      ]);

      return {
        channel: resolveBuildChannel(identifier),
        version,
        hash: hash.status === "ok" ? hash.data.slice(0, 7) : null,
      };
    },
  }).data;
}

async function openDevtoolsPanel() {
  const result = await windowsCommands.devtoolsPanelShow();
  if (result.status === "error") {
    console.error("Failed to show Devtools panel:", result.error);
  }
}

function Segment(props: {
  children: React.ReactNode;
  label: string;
  title: string;
}) {
  ignoreReactScan(props);

  return (
    <div
      title={props.title}
      className="flex items-center gap-1.5 border-l border-white/15 px-2"
    >
      <span className={LABEL_CLASS}>{props.label}</span>
      {props.children}
    </div>
  );
}

function BarButton(props: {
  children: React.ReactNode;
  onClick: () => void;
  title: string;
}) {
  ignoreReactScan(props);

  return (
    <button
      type="button"
      title={props.title}
      onClick={props.onClick}
      className={cn([
        "flex items-center gap-1.5 border-l border-white/15 px-2 first:border-l-0",
        "hover:bg-white/10 focus-visible:bg-white/10 focus-visible:outline-hidden",
      ])}
    >
      {props.children}
    </button>
  );
}

const SPARKLINE_WIDTH = 36;
const SPARKLINE_HEIGHT = 10;

/**
 * `floor` pins the top of the chart to at least that value (e.g. 60 fps).
 * `relative` scales between the window's min and max so slow drifts such as
 * memory growth stay visible instead of flattening against zero.
 */
function Sparkline(props: {
  values: number[];
  floor?: number;
  relative?: boolean;
}) {
  ignoreReactScan(props);
  const { values, floor = 1, relative = false } = props;

  if (values.length < 2) {
    return (
      <svg
        aria-hidden
        width={SPARKLINE_WIDTH}
        height={SPARKLINE_HEIGHT}
        className="shrink-0"
      />
    );
  }

  const min = relative ? Math.min(...values) : 0;
  const max = relative ? Math.max(...values) : Math.max(floor, ...values);
  const range = max - min;
  const step = SPARKLINE_WIDTH / (HISTORY_LENGTH - 1);
  const offset = SPARKLINE_WIDTH - (values.length - 1) * step;
  const points = values
    .map((value, index) => {
      const x = offset + index * step;
      const ratio = range === 0 ? 0.5 : (value - min) / range;
      const y = SPARKLINE_HEIGHT - 0.5 - ratio * (SPARKLINE_HEIGHT - 1);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  return (
    <svg
      aria-hidden
      width={SPARKLINE_WIDTH}
      height={SPARKLINE_HEIGHT}
      className="shrink-0 opacity-80"
    >
      <polyline
        fill="none"
        points={points}
        stroke="currentColor"
        strokeLinejoin="round"
        strokeWidth="1"
      />
    </svg>
  );
}

function last(values: number[]): number | undefined {
  return values.length ? values[values.length - 1] : undefined;
}

function average(values: number[]): number {
  if (!values.length) return 0;
  return Math.round(
    values.reduce((sum, value) => sum + value, 0) / values.length,
  );
}
