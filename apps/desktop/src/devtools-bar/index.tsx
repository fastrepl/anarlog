import { useQuery } from "@tanstack/react-query";
import { getIdentifier, getVersion } from "@tauri-apps/api/app";
import { useReducer, useState } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import {
  DEVTOOLS_MENU,
  type DevtoolsAction,
  useDevtoolsActions,
} from "./actions";
import { copyDiagnostics } from "./diagnostics";
import { Hint } from "./hint";
import { MenuGroup, MenuHint } from "./menu";
import {
  type DevtoolsMetrics,
  formatBytes,
  getTopIpcCommands,
  HISTORY_LENGTH,
  startDevtoolsMetrics,
  useDevtoolsMetrics,
} from "./metrics";
import { QuickSettingsMenu } from "./quick-settings";
import {
  areRenderOutlinesEnabled,
  getTopRenderedComponents,
  ignoreRenderTracking,
  setRenderOutlinesEnabled,
} from "./render-tracker";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { commands } from "~/types/tauri.gen";

export type BuildChannel = "dev" | "staging" | "stable";

export function resolveBuildChannel(identifier: string): BuildChannel {
  if (identifier.endsWith(".staging")) return "staging";
  if (identifier.endsWith(".dev")) return "dev";
  return "stable";
}

const CHANNEL_DOT: Record<BuildChannel, string> = {
  dev: "bg-sky-400",
  staging: "bg-amber-400",
  stable: "bg-neutral-400",
};

const CHANNEL_TEXT: Record<BuildChannel, string> = {
  dev: "text-sky-300",
  staging: "text-amber-300",
  stable: "text-neutral-200",
};

const COLLAPSED_STORAGE_KEY = "anarlog:devtools-bar:collapsed";

const ITEM_CLASS = "flex items-center gap-1.5 px-2";
const BUTTON_CLASS = cn([
  ITEM_CLASS,
  "hover:bg-white/8 focus-visible:bg-white/8 focus-visible:outline-hidden",
  "data-[state=open]:bg-white/8",
]);
const LABEL_CLASS = "text-neutral-500";
const VALUE_CLASS = "tabular-nums text-neutral-100";

type Tone = "ok" | "warn" | "bad";

const TONE_CLASS: Record<Tone, string> = {
  ok: "text-neutral-100",
  warn: "text-amber-400",
  bad: "text-red-400",
};

/**
 * Linear-style status bar for dev and staging builds. Stable builds never
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
  const [collapsed, setCollapsed] = useState(readCollapsed);

  if (!build) {
    return null;
  }

  const toggleCollapsed = () => {
    setCollapsed((current) => {
      const next = !current;
      try {
        localStorage.setItem(COLLAPSED_STORAGE_KEY, next ? "1" : "0");
      } catch {}
      return next;
    });
  };

  if (collapsed) {
    return (
      <>
        <button
          type="button"
          aria-label="Expand developer bar"
          title="Expand developer bar"
          data-testid="devtools-status-bar-collapsed"
          className={cn([
            "h-1.5 w-full shrink-0 border-t border-neutral-800 bg-neutral-900",
            "hover:bg-neutral-700 focus-visible:outline-hidden",
          ])}
          onClick={toggleCollapsed}
        >
          <span
            className={cn([
              "mx-auto block h-0.5 w-8 rounded-full",
              CHANNEL_DOT[build.channel],
            ])}
          />
        </button>
        {dialogs}
      </>
    );
  }

  return (
    <>
      <footer
        aria-label="Developer status bar"
        data-testid="devtools-status-bar"
        className={cn([
          "flex h-6 shrink-0 items-stretch select-none",
          "border-t border-neutral-800 bg-neutral-900 text-neutral-300",
          "font-mono text-[11px] leading-none",
        ])}
      >
        <DevtoolsMenu onAction={run}>
          <button
            type="button"
            title="Devtools actions"
            className={cn([BUTTON_CLASS, "pl-2.5"])}
          >
            <span
              className={cn([
                "size-1.5 rounded-full",
                CHANNEL_DOT[build.channel],
              ])}
            />
            <span
              className={cn([
                "font-semibold tracking-wider uppercase",
                CHANNEL_TEXT[build.channel],
              ])}
            >
              {build.channel}
            </span>
            <span className="text-neutral-500">
              {build.version}
              {build.hash ? ` ${build.hash}` : ""}
            </span>
          </button>
        </DevtoolsMenu>
        <PlanBadge />

        <div className="flex min-w-0 flex-1 items-stretch overflow-hidden">
          <LiveMetrics />
        </div>

        <Hint content="Copy a diagnostics snapshot (build, device, metrics, top commands and components, sync) as JSON">
          <button
            type="button"
            aria-label="Copy diagnostics"
            className={BUTTON_CLASS}
            onClick={() => void copyDiagnostics()}
          >
            ↓
          </button>
        </Hint>
        <Hint content="Collapse the developer bar">
          <button
            type="button"
            aria-label="Collapse developer bar"
            className={cn([BUTTON_CLASS, "pr-2.5"])}
            onClick={toggleCollapsed}
          >
            _
          </button>
        </Hint>
      </footer>
      {dialogs}
    </>
  );
}

function readCollapsed(): boolean {
  try {
    return localStorage.getItem(COLLAPSED_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

function PlanBadge() {
  const { session } = useAuth();
  const billing = useBillingAccess();

  if (!session) {
    return (
      <div className={cn([ITEM_CLASS, "text-neutral-500"])}>signed out</div>
    );
  }

  if (!billing.isReady) {
    return null;
  }

  const label =
    billing.plan === "trial" && billing.trialDaysRemaining !== null
      ? `trial ${billing.trialDaysRemaining}d`
      : billing.isLite
        ? "lite"
        : billing.plan;

  return (
    <Hint
      content={
        <Rows
          rows={[
            ["plan", billing.plan],
            ["subscription", billing.subscriptionStatus ?? "none"],
            ["payment method", billing.hasPaymentMethod ? "yes" : "no"],
            ["entitlements", billing.entitlements.join(", ") || "none"],
          ]}
        />
      }
    >
      <div className={cn([ITEM_CLASS, "text-neutral-400"])}>{label}</div>
    </Hint>
  );
}

// Isolated so the once-per-second metrics tick only re-renders the metrics,
// not the menu and dialogs above.
function LiveMetrics() {
  const metrics = useDevtoolsMetrics();
  const [, refresh] = useReducer((tick: number) => tick + 1, 0);

  const fps = last(metrics.fps);
  const jank = last(metrics.jank) ?? 0;
  const delay = last(metrics.delay) ?? 0;
  const invokes = last(metrics.invokes) ?? 0;
  const callbacks = last(metrics.callbacks) ?? 0;
  const renders = last(metrics.renders) ?? 0;
  const memoryBytes = last(metrics.memoryBytes);
  const outlinesEnabled = areRenderOutlinesEnabled();
  const fpsTarget = Math.max(60, ...metrics.fps);

  return (
    <>
      <Metric
        label="FPS"
        value={fps ?? "–"}
        tone={
          fps === undefined
            ? "ok"
            : toneBelow(fps, fpsTarget * 0.8, fpsTarget * 0.5)
        }
        chart={<Sparkline values={metrics.fps} floor={60} />}
        tooltip={
          <Rows
            title="Frames per second"
            rows={[
              ["avg", `${average(metrics.fps)}`],
              ["min", `${Math.min(...metrics.fps, fps ?? 0)}`],
              ["window", `${metrics.fps.length}s`],
            ]}
          />
        }
      />
      <Metric
        label="Jank"
        value={`${jank}%`}
        tone={toneAbove(jank, 3, 10)}
        chart={<Bars values={metrics.jank} ceiling={20} />}
        tooltip={
          <Rows
            title="Frames over 34ms (a dropped frame at 60Hz)"
            rows={[
              ["avg", `${average(metrics.jank)}%`],
              ["max", `${Math.max(...metrics.jank, 0)}%`],
            ]}
          />
        }
      />
      <Metric
        label="Delay"
        value={`${delay}ms`}
        tone={toneAbove(delay, 60, 200)}
        tooltip={
          <Rows
            title="Worst main-thread stall in the last second"
            rows={[
              ["avg", `${average(metrics.delay)}ms`],
              ["max", `${Math.max(...metrics.delay, 0)}ms`],
            ]}
          />
        }
      />
      <Metric
        as="button"
        label="Renders"
        value={renders}
        chart={<Sparkline values={metrics.renders} />}
        trailing={
          <span
            className={outlinesEnabled ? "text-violet-300" : "text-neutral-500"}
          >
            {outlinesEnabled ? "◉" : "○"}
          </span>
        }
        onClick={() => {
          setRenderOutlinesEnabled(!outlinesEnabled);
          refresh();
        }}
        tooltip={
          <Rows
            title={`React renders per second · outlines ${outlinesEnabled ? "on" : "off"} (click to toggle)`}
            rows={getTopRenderedComponents().map(({ name, count }) => [
              name,
              `×${count}`,
            ])}
            empty="No renders in the last 10s"
          />
        }
      />
      <Metric
        label="IPC"
        value={`↑${invokes} ↓${callbacks}`}
        chart={
          <Sparkline
            values={metrics.invokes.map(
              (value, index) => value + (metrics.callbacks[index] ?? 0),
            )}
          />
        }
        tooltip={
          <Rows
            title="Tauri IPC per second · ↑ invokes · ↓ callbacks (responses, events, channels)"
            rows={getTopIpcCommands().map(({ command, count }) => [
              command,
              `×${count}`,
            ])}
            empty="No invokes in the last 10s"
          />
        }
      />
      <Metric
        label="Net"
        value={metrics.requestsInFlight}
        tooltip={
          <Rows
            title="HTTP requests in flight (IPC excluded)"
            rows={[
              ["last second", `${last(metrics.requests) ?? 0}`],
              [
                "window total",
                `${metrics.requests.reduce((a, b) => a + b, 0)}`,
              ],
            ]}
          />
        }
      />
      <Metric
        label="Mem"
        value={memoryBytes === undefined ? "–" : formatBytes(memoryBytes)}
        chart={<Sparkline values={metrics.memoryBytes} relative />}
        tooltip={
          <Rows
            title="Host process resident memory (WebKit renders in a separate process on macOS)"
            rows={[
              [
                "min",
                formatBytes(Math.min(...metrics.memoryBytes, memoryBytes ?? 0)),
              ],
              ["max", formatBytes(Math.max(...metrics.memoryBytes, 0))],
              ["change", describeChange(metrics.memoryBytes)],
            ]}
          />
        }
      />
    </>
  );
}

function describeChange(history: DevtoolsMetrics["memoryBytes"]): string {
  if (history.length < 2) return "–";
  const delta = history[history.length - 1]! - history[0]!;
  const sign = delta >= 0 ? "+" : "−";
  return `${sign}${formatBytes(Math.abs(delta))} over ${history.length * 2}s`;
}

function Metric(props: {
  as?: "div" | "button";
  label: string;
  value: React.ReactNode;
  tone?: Tone;
  chart?: React.ReactNode;
  trailing?: React.ReactNode;
  onClick?: () => void;
  tooltip: React.ReactNode;
}) {
  const {
    as = "div",
    label,
    value,
    tone = "ok",
    chart,
    trailing,
    onClick,
  } = props;

  const content = (
    <>
      <span className={LABEL_CLASS}>{label}</span>
      <span className={cn([VALUE_CLASS, TONE_CLASS[tone]])}>{value}</span>
      {chart}
      {trailing}
    </>
  );

  return (
    <Hint content={props.tooltip}>
      {as === "button" ? (
        <button type="button" className={BUTTON_CLASS} onClick={onClick}>
          {content}
        </button>
      ) : (
        <div className={ITEM_CLASS}>{content}</div>
      )}
    </Hint>
  );
}

function Rows(props: {
  title?: string;
  rows: Array<[string, string]>;
  empty?: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      {props.title ? (
        <div className="max-w-72 text-neutral-400">{props.title}</div>
      ) : null}
      {props.rows.length ? (
        <table className="tabular-nums">
          <tbody>
            {props.rows.map(([key, value]) => (
              <tr key={key}>
                <td className="max-w-64 truncate pr-3 text-neutral-300">
                  {key}
                </td>
                <td className="text-right text-neutral-100">{value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : props.empty ? (
        <div className="text-neutral-500">{props.empty}</div>
      ) : null}
    </div>
  );
}

function DevtoolsMenu(props: {
  children: React.ReactNode;
  onAction: (action: DevtoolsAction) => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>{props.children}</DropdownMenuTrigger>
      <DropdownMenuContent align="start" side="top" className="w-48">
        <QuickSettingsMenu />
        <DropdownMenuSeparator />
        {DEVTOOLS_MENU.map((group) => (
          <MenuGroup
            key={group.label}
            label={group.label}
            description={group.description}
          >
            {group.items.map((item) => (
              <MenuHint key={item.action} description={item.description}>
                <DropdownMenuItem
                  className={cn([item.destructive && "text-destructive"])}
                  onSelect={() => props.onAction(item.action)}
                >
                  {item.label}
                </DropdownMenuItem>
              </MenuHint>
            ))}
          </MenuGroup>
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

function toneAbove(value: number, warn: number, bad: number): Tone {
  if (value > bad) return "bad";
  if (value > warn) return "warn";
  return "ok";
}

function toneBelow(value: number, warn: number, bad: number): Tone {
  if (value < bad) return "bad";
  if (value < warn) return "warn";
  return "ok";
}

const CHART_WIDTH = 36;
const CHART_HEIGHT = 10;

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
  const { values, floor = 1, relative = false } = props;

  if (values.length < 2) {
    return <ChartFrame />;
  }

  const min = relative ? Math.min(...values) : 0;
  const max = relative ? Math.max(...values) : Math.max(floor, ...values);
  const range = max - min;
  const step = CHART_WIDTH / (HISTORY_LENGTH - 1);
  const offset = CHART_WIDTH - (values.length - 1) * step;
  const points = values
    .map((value, index) => {
      const x = offset + index * step;
      const ratio = range === 0 ? 0.5 : (value - min) / range;
      const y = CHART_HEIGHT - 0.5 - ratio * (CHART_HEIGHT - 1);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");

  return (
    <ChartFrame>
      <polyline
        fill="none"
        points={points}
        stroke="currentColor"
        strokeLinejoin="round"
        strokeWidth="1"
      />
    </ChartFrame>
  );
}

/** Bar chart clamped to `ceiling`, so small percentages still register. */
function Bars(props: { values: number[]; ceiling: number }) {
  const { values, ceiling } = props;
  const step = CHART_WIDTH / HISTORY_LENGTH;
  const offset = CHART_WIDTH - values.length * step;

  return (
    <ChartFrame>
      {values.map((value, index) => {
        const height = Math.max(
          1,
          (Math.min(value, ceiling) / ceiling) * CHART_HEIGHT,
        );
        return (
          <rect
            key={index}
            x={(offset + index * step).toFixed(1)}
            y={(CHART_HEIGHT - height).toFixed(1)}
            width={Math.max(step - 0.4, 0.4).toFixed(1)}
            height={height.toFixed(1)}
            fill="currentColor"
            opacity={value > 0 ? 1 : 0.25}
          />
        );
      })}
    </ChartFrame>
  );
}

function ChartFrame({ children }: { children?: React.ReactNode }) {
  return (
    <svg
      aria-hidden
      width={CHART_WIDTH}
      height={CHART_HEIGHT}
      className="shrink-0 text-neutral-400"
    >
      {children}
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
