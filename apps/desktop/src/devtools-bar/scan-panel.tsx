import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { cn } from "@anlg/utils";

import { useDevtoolsMetrics } from "./metrics";
import type { ReactToolsState } from "./react-tools";
import {
  setReactInspecting,
  setReactToolbarVisible,
  setReactScanSettings,
} from "./react-tools";
import type { PromptMode, ScanEvent } from "./scan-data";
import {
  clearScanHistory,
  getScanPrompt,
  mountScanInspector,
  setScanAlerts,
  useScanData,
} from "./scan-data";
import { RESIZE_HANDLES, useScanPanelLayout } from "./scan-panel-layout";

const buttonClass =
  "rounded-lg px-2 py-1 text-muted-foreground hover:bg-background/50 hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring";
type View = "ranked" | "overview" | "prompts" | "settings";
type Selection = Readonly<{
  event: ScanEvent;
  prompts: Record<PromptMode, string>;
}>;
function captureSelection(event: ScanEvent): Selection {
  return {
    event,
    prompts: {
      fix: getScanPrompt(event.id, "fix"),
      explanation: getScanPrompt(event.id, "explanation"),
      data: getScanPrompt(event.id, "data"),
    },
  };
}

export function ScanPanel({
  state,
  copyText,
}: Readonly<{
  state: ReactToolsState;
  copyText?: (text: string) => Promise<void>;
}>) {
  const data = useScanData();
  const layout = useScanPanelLayout();
  const { panelRef } = layout;
  useEffect(() => {
    panelRef.current?.showPopover?.();
  }, [panelRef]);
  const [view, setView] = useState<View>("ranked");
  // Retain only the user's selected event and its exact prompts when the
  // bounded live buffer rolls over. Clear explicitly releases the selection.
  const [selection, setSelection] = useState<Selection | null>(null);
  const selected = selection?.event ?? data.events[0];
  return createPortal(
    <section
      ref={layout.panelRef}
      popover="manual"
      style={{ ...layout.style, position: "fixed", margin: 0 }}
      {...layout.handlers}
      data-devbar=""
      data-testid="react-scan-panel"
      aria-label="React Scan performance"
      className="border-border bg-background text-foreground flex min-h-0 flex-col overflow-hidden rounded-[22px] border font-mono text-[11px] leading-snug shadow-xl"
    >
      <div className="flex min-h-0 flex-1">
        <aside
          aria-label="Slowdown history"
          className="bg-background/15 border-border/40 flex w-[28%] max-w-[300px] min-w-[180px] shrink-0 flex-col border-r"
        >
          <div
            onPointerDown={layout.drag}
            className="border-border/40 flex h-10 shrink-0 cursor-move touch-none items-center gap-2 border-b pr-3 pl-6"
          >
            <span className="min-w-0 flex-1 truncate">
              History{" "}
              {state.settings.showNotificationCount
                ? `(${data.events.length})`
                : ""}
            </span>
            <button
              type="button"
              className={buttonClass}
              disabled={!data.events.length}
              onClick={() => {
                setSelection(null);
                clearScanHistory();
              }}
            >
              Clear
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-2">
            {data.events.map((event) => (
              <button
                key={event.id}
                type="button"
                aria-pressed={event.id === selected?.id}
                title={event.label}
                className={cn([
                  "hover:bg-background/50 mb-0.5 flex w-full items-center gap-2 rounded-xl px-2 py-2 text-left",
                  event.id === selected?.id &&
                    "bg-background/60 ring-border/30 shadow-sm ring-1",
                ])}
                onClick={() => {
                  setSelection(captureSelection(event));
                  setReactInspecting(false);
                }}
              >
                <span
                  aria-hidden="true"
                  className={cn([
                    "size-1.5 shrink-0 rounded-full",
                    event.severity === "high"
                      ? "bg-destructive"
                      : event.severity === "needs-improvement"
                        ? "bg-amber-500"
                        : "bg-blue-500",
                  ])}
                />
                <span className="min-w-0 flex-1 truncate">{event.label}</span>
                <span className="text-muted-foreground shrink-0 tabular-nums">
                  {event.fps === null
                    ? `${ms(event.duration)}ms`
                    : `${Math.round(event.fps)} FPS`}
                </span>
              </button>
            ))}
            {!data.events.length ? (
              <p className="text-muted-foreground px-2 py-3">
                Interact with the app to capture clicks, typing, and frame
                drops.
              </p>
            ) : null}
          </div>
          <div className="border-border/40 flex shrink-0 items-center justify-between gap-1 border-t px-3 py-2">
            {state.settings.showFPS ? <PanelFps /> : null}
            <button
              type="button"
              aria-pressed={data.alertsEnabled}
              className={cn([
                buttonClass,
                data.alertsEnabled &&
                  "bg-background/65 text-foreground ring-border/30 shadow-sm ring-1",
              ])}
              onClick={() => setScanAlerts(!data.alertsEnabled)}
            >
              Alerts {data.alertsEnabled ? "on" : "off"}
            </button>
          </div>
        </aside>
        <div className="flex min-w-0 flex-1 flex-col">
          <div
            onPointerDown={layout.drag}
            className="border-border/40 flex h-10 shrink-0 cursor-move touch-none items-center gap-1 border-b pr-6 pl-3"
          >
            <div
              role="tablist"
              aria-label="Performance views"
              className="flex min-w-0 items-center gap-1"
            >
              {(["ranked", "overview", "prompts", "settings"] as const).map(
                (tab) => (
                  <button
                    key={tab}
                    type="button"
                    role="tab"
                    aria-selected={!state.inspecting && view === tab}
                    className={cn([
                      buttonClass,
                      !state.inspecting &&
                        view === tab &&
                        "bg-background/65 text-foreground ring-border/30 shadow-sm ring-1",
                    ])}
                    onClick={() => {
                      setReactInspecting(false);
                      if (selected && !selection)
                        setSelection(captureSelection(selected));
                      setView(tab);
                    }}
                  >
                    {tab[0].toUpperCase() + tab.slice(1)}
                  </button>
                ),
              )}
            </div>
            <button
              type="button"
              aria-label="Close React Scan panel"
              className={buttonClass}
              onClick={() => setReactToolbarVisible(false)}
            >
              ×
            </button>
          </div>
          {view === "settings" && !state.inspecting ? (
            <div className="space-y-3 overflow-auto p-3">
              <p>React Scan {state.version}</p>
              <label className="flex items-center justify-between gap-3">
                Animation speed
                <select
                  aria-label="Animation speed"
                  value={state.settings.animationSpeed}
                  onChange={(event) => {
                    const animationSpeed = event.target.value;
                    if (
                      animationSpeed === "fast" ||
                      animationSpeed === "slow" ||
                      animationSpeed === "off"
                    )
                      setReactScanSettings({ animationSpeed });
                  }}
                >
                  <option value="fast">Fast</option>
                  <option value="slow">Slow</option>
                  <option value="off">Off</option>
                </select>
              </label>
              {(
                [
                  ["log", "Log renders to console"],
                  ["showFPS", "Show panel FPS"],
                  ["showNotificationCount", "Show slowdown count"],
                ] as const
              ).map(([key, label]) => (
                <label
                  key={key}
                  className="flex items-center justify-between gap-3"
                >
                  {label}
                  <input
                    type="checkbox"
                    checked={state.settings[key]}
                    onChange={(event) =>
                      setReactScanSettings({ [key]: event.target.checked })
                    }
                  />
                </label>
              ))}
              <p className="text-muted-foreground">
                Console logging and render outlines add profiling overhead.
                Browser timing support varies; unavailable event timings are not
                evidence of zero work.
              </p>
            </div>
          ) : state.inspecting ? (
            <Inspector />
          ) : selected ? (
            <div
              role="tabpanel"
              aria-label={view}
              className="flex min-h-0 flex-1 flex-col"
            >
              <div className="border-border/40 flex shrink-0 items-center gap-2 border-b px-3 py-2">
                <span
                  className="truncate font-semibold"
                  title={selected.path.join(" → ")}
                >
                  {selected.label}
                </span>
                <span className="text-muted-foreground ml-auto shrink-0 tabular-nums">
                  {ms(selected.duration)}ms
                  {selected.fps === null
                    ? " processing"
                    : ` · ${Math.round(selected.fps)} FPS`}
                </span>
              </div>
              {view === "prompts" ? (
                <Prompts
                  key={selected.id}
                  prompts={(selection ?? captureSelection(selected)).prompts}
                  copyText={copyText}
                />
              ) : (
                <div className="min-h-0 flex-1 overflow-auto p-3">
                  {view === "ranked" ? (
                    <Ranked event={selected} />
                  ) : (
                    <Overview event={selected} />
                  )}
                </div>
              )}
            </div>
          ) : (
            <div className="text-muted-foreground p-4">
              Select a captured event to inspect its timings and copy a
              performance prompt.
            </div>
          )}
        </div>
      </div>
      {RESIZE_HANDLES.map(([edge, className]) => (
        <div
          key={edge}
          data-resize-edge={edge}
          aria-hidden="true"
          onPointerDown={(event) => layout.resize(event, edge)}
          className={cn(["absolute z-10 touch-none", className])}
        >
          {edge === "se" ? (
            <span className="text-muted-foreground pointer-events-none absolute right-2 bottom-2">
              ◢
            </span>
          ) : null}
        </div>
      ))}
    </section>,
    document.body,
  );
}

function Prompts({
  prompts,
  copyText,
}: Readonly<{
  prompts: Record<PromptMode, string>;
  copyText?: (text: string) => Promise<void>;
}>) {
  const [mode, setMode] = useState<PromptMode>("fix");
  const [copied, setCopied] = useState<string | null>(null);
  const [error, setError] = useState(false);
  const prompt = prompts[mode];
  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 p-3">
      <div
        className="flex items-center gap-1"
        role="tablist"
        aria-label="Prompt type"
      >
        {(["fix", "explanation", "data"] as const).map((tab) => (
          <button
            type="button"
            role="tab"
            key={tab}
            aria-selected={mode === tab}
            className={cn([
              buttonClass,
              mode === tab &&
                "bg-background/65 text-foreground ring-border/30 shadow-sm ring-1",
            ])}
            onClick={() => {
              setMode(tab);
              setError(false);
            }}
          >
            {tab[0].toUpperCase() + tab.slice(1)}
          </button>
        ))}
        <button
          type="button"
          disabled={!prompt || !copyText}
          className="border-border/40 bg-background/55 hover:bg-background/50 ml-auto rounded-lg border px-3 py-1 font-semibold shadow-sm disabled:opacity-50"
          onClick={() => {
            setError(false);
            void copyText?.(prompt).then(
              () => setCopied(prompt),
              () => setError(true),
            );
          }}
        >
          {copied === prompt ? "Copied" : "Copy Prompt"}
        </button>
      </div>
      <textarea
        aria-label={`${mode} prompt`}
        readOnly
        value={prompt}
        className="border-border/40 bg-background/55 focus:border-ring min-h-0 flex-1 resize-none rounded-xl border p-3 text-[11px] leading-relaxed outline-none"
      />
      <p role="status" className="text-muted-foreground shrink-0">
        {error
          ? "Could not copy. Select the prompt text and copy it manually."
          : prompt
            ? "React Scan's prompt for this event. Paste it into your coding assistant."
            : "This event has left the capture buffer. Select a more recent event."}
      </p>
    </div>
  );
}

function Ranked({ event }: Readonly<{ event: ScanEvent }>) {
  const rows = [
    ...event.components.map((component) => ({
      label: component.name,
      time: component.time,
      component,
    })),
    ...event.timings
      .filter((timing) => timing.label !== "React renders")
      .map((timing) => ({ ...timing, component: null })),
  ].sort((a, b) => b.time - a.time);
  const max = Math.max(1, ...rows.map((row) => row.time));
  return (
    <div className="space-y-2">
      {rows.map((row, index) => (
        <details
          key={`${row.label}-${index}`}
          className="border-border/40 bg-background/25 overflow-hidden rounded-xl border"
          open={row.component ? undefined : false}
        >
          <summary
            className={cn([
              "relative flex items-center gap-2 overflow-hidden px-3 py-2",
              row.component ? "cursor-pointer" : "pointer-events-none",
            ])}
          >
            <span
              aria-hidden="true"
              className="bg-foreground/5 absolute inset-y-0 left-0"
              style={{ width: `${(Math.max(0, row.time) / max) * 100}%` }}
            />
            <span className="relative min-w-0 flex-1 truncate">
              {row.label}
            </span>
            {row.component ? (
              <span className="text-muted-foreground relative">
                {row.component.renders} renders
              </span>
            ) : null}
            <span className="relative tabular-nums">{ms(row.time)}ms</span>
          </summary>
          {row.component ? (
            <div className="border-border/40 text-muted-foreground space-y-1 border-t px-3 py-2">
              <p>
                {row.component.mounted
                  ? "Includes mounting renders"
                  : "Update renders"}
                {row.component.compiled ? " · React Compiler" : ""}
              </p>
              {row.component.changes.length ? (
                row.component.changes.map((change) => (
                  <p key={`${change.kind}-${change.name}`}>
                    {change.kind}{" "}
                    <span className="text-foreground">{change.name}</span>{" "}
                    changed {change.count}×
                  </p>
                ))
              ) : (
                <p>No prop, state, or context changes recorded.</p>
              )}
            </div>
          ) : null}
        </details>
      ))}
      <p className="text-muted-foreground pt-2">
        Expand a component for its render reasons. Browser work is listed
        alongside React renders.
      </p>
    </div>
  );
}

function Overview({ event }: Readonly<{ event: ScanEvent }>) {
  return (
    <div className="space-y-4">
      <div className="flex items-baseline justify-between">
        <span>What was time spent on?</span>
        <strong className="tabular-nums">{ms(event.duration)}ms total</strong>
      </div>
      {event.timings.map((timing) => (
        <div key={timing.label} className="space-y-1.5">
          <div className="flex justify-between gap-3">
            <span>{timing.label}</span>
            <span className="text-muted-foreground tabular-nums">
              {ms(timing.time)}ms ·{" "}
              {Math.round(
                (Math.max(0, timing.time) / Math.max(1, event.duration)) * 100,
              )}
              %
            </span>
          </div>
          <div className="bg-foreground/10 h-1.5 overflow-hidden rounded-full">
            <div
              className="h-full rounded bg-blue-500"
              style={{
                width: `${Math.min(100, (Math.max(0, timing.time) / Math.max(1, event.duration)) * 100)}%`,
              }}
            />
          </div>
        </div>
      ))}
      <p className="text-muted-foreground">
        Timings are reported by React Scan. Development profiling and render
        outlines can add overhead.
      </p>
    </div>
  );
}

function Inspector() {
  const host = useRef<HTMLDivElement>(null);
  useEffect(
    () => (host.current ? mountScanInspector(host.current) : undefined),
    [],
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <p className="border-border/40 text-muted-foreground shrink-0 border-b px-3 py-2">
        Select a component in the app to inspect props, state, context, and
        render changes. Esc releases selection.
      </p>
      <div ref={host} className="min-h-0 flex-1" />
    </div>
  );
}
function PanelFps() {
  const fps = useDevtoolsMetrics(
    (metrics) => metrics.fps[metrics.fps.length - 1],
  );
  return (
    <span className="text-muted-foreground inline-block w-[8ch] shrink-0 truncate text-right tabular-nums">
      {fps === undefined ? "–" : Math.round(fps)} FPS
    </span>
  );
}
function ms(value: number): string {
  return Math.max(0, value).toFixed(value < 10 ? 1 : 0);
}
