import { useQuery } from "@tanstack/react-query";
import { getIdentifier, getVersion } from "@tauri-apps/api/app";
import { useReducer } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import {
  DEVTOOLS_MENU,
  type DevtoolsAction,
  useDevtoolsActions,
} from "./actions";
import {
  formatBytes,
  HISTORY_LENGTH,
  startDevtoolsMetrics,
  useDevtoolsMetrics,
} from "./metrics";
import {
  areRenderOutlinesEnabled,
  getTopRenderedComponents,
  ignoreRenderTracking,
  setRenderOutlinesEnabled,
} from "./render-tracker";

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
const BAR_ITEM_CLASS =
  "flex items-center gap-1.5 border-l border-white/15 px-2";
const BAR_BUTTON_CLASS = cn([
  BAR_ITEM_CLASS,
  "first:border-l-0",
  "hover:bg-white/10 focus-visible:bg-white/10 focus-visible:outline-hidden",
  "data-[state=open]:bg-white/10",
]);

/**
 * VS Code-style status bar for dev and staging builds. Stable builds never
 * render it (`show_devtool` is false there), so copy stays English-only.
 */
export function DevtoolsStatusBar(props: Record<never, never>) {
  ignoreRenderTracking(props);

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
  ignoreRenderTracking(props);
  useMountEffect(() => startDevtoolsMetrics());

  const build = useBuildInfo();
  const { dialogs, run } = useDevtoolsActions();

  if (!build) {
    return null;
  }

  const channel = build.channel;

  return (
    <>
      <footer
        aria-label="Developer status bar"
        data-testid="devtools-status-bar"
        className={cn([
          "flex h-6 shrink-0 items-stretch overflow-hidden select-none",
          "font-mono text-[11px] leading-none",
          CHANNEL_CLASSES[channel],
        ])}
      >
        <DevtoolsMenu onAction={run}>
          <button
            type="button"
            title="Devtools actions"
            className={BAR_BUTTON_CLASS}
          >
            <span className="font-semibold tracking-wider uppercase">
              {channel}
            </span>
            <span className="opacity-70">
              {build.version}
              {build.hash ? ` ${build.hash}` : ""}
            </span>
          </button>
        </DevtoolsMenu>
        <LiveMetrics />
      </footer>
      {dialogs}
    </>
  );
}

// Isolated so the once-per-second metrics tick does not re-render the menu
// and dialogs above, which would show up in the render counter itself.
function LiveMetrics(props: Record<never, never>) {
  ignoreRenderTracking(props);

  const metrics = useDevtoolsMetrics();
  const [, refresh] = useReducer((tick: number) => tick + 1, 0);

  const fps = last(metrics.fps);
  const invokes = last(metrics.invokes) ?? 0;
  const callbacks = last(metrics.callbacks) ?? 0;
  const renders = last(metrics.renders) ?? 0;
  const memoryBytes = last(metrics.memoryBytes);
  const outlinesEnabled = areRenderOutlinesEnabled();

  return (
    <>
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

      <button
        type="button"
        title={describeRenders(outlinesEnabled)}
        className={BAR_BUTTON_CLASS}
        onClick={() => {
          setRenderOutlinesEnabled(!outlinesEnabled);
          refresh();
        }}
      >
        <span className={LABEL_CLASS}>renders</span>
        <span className={VALUE_CLASS}>{renders}</span>
        <Sparkline values={metrics.renders} />
        <span className={cn(["opacity-70", !outlinesEnabled && "opacity-40"])}>
          {outlinesEnabled ? "◉" : "○"} outline
        </span>
      </button>

      <Segment
        label="MEM"
        title="Host process resident memory (WebKit content process is separate on macOS)"
      >
        <span className={VALUE_CLASS}>
          {memoryBytes === undefined ? "–" : formatBytes(memoryBytes)}
        </span>
        <Sparkline values={metrics.memoryBytes} relative />
      </Segment>
    </>
  );
}

function describeRenders(outlinesEnabled: boolean): string {
  const top = getTopRenderedComponents()
    .map(({ name, count }) => `${name} ×${count}`)
    .join("\n");

  return [
    `React component renders per second. Outlines ${outlinesEnabled ? "on" : "off"} (click to toggle).`,
    top ? `Most rendered (last 10s):\n${top}` : null,
  ]
    .filter(Boolean)
    .join("\n\n");
}

function DevtoolsMenu(props: {
  children: React.ReactNode;
  onAction: (action: DevtoolsAction) => void;
}) {
  ignoreRenderTracking(props);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>{props.children}</DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top" className="w-48">
        {DEVTOOLS_MENU.map((group) => (
          <DropdownMenuSub key={group.label}>
            <DropdownMenuSubTrigger>{group.label}</DropdownMenuSubTrigger>
            <DropdownMenuSubContent className="w-56">
              {group.items.map((item) => (
                <DropdownMenuItem
                  key={item.action}
                  className={cn([item.destructive && "text-destructive"])}
                  onSelect={() => props.onAction(item.action)}
                >
                  {item.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenuSubContent>
          </DropdownMenuSub>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
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

function Segment(props: {
  children: React.ReactNode;
  label: string;
  title: string;
}) {
  ignoreRenderTracking(props);

  return (
    <div title={props.title} className={BAR_ITEM_CLASS}>
      <span className={LABEL_CLASS}>{props.label}</span>
      {props.children}
    </div>
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
  ignoreRenderTracking(props);
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
