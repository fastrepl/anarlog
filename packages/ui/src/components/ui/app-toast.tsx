import {
  AnimatePresence,
  motion,
  useReducedMotion,
  type PanInfo,
} from "motion/react";
import {
  useId,
  useState,
  useSyncExternalStore,
  type CSSProperties,
  type FocusEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";

import {
  CaretDown,
  CheckCircle,
  Info,
  WarningCircle,
  X,
} from "@anlg/ui/components/icons";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { panelSquircle } from "@anlg/ui/lib/squircle";
import { appToastSwipeDismissDirection } from "@anlg/ui/lib/toast-gesture";
import { cn } from "@anlg/utils";

export type AppToastTone = "info" | "success" | "warning" | "error";

export type AppToastAction = Readonly<{
  label: ReactNode;
  onClick: (event: ReactMouseEvent<HTMLButtonElement>) => void;
  /** Leave the toast up after `onClick` so the caller can morph it. Defaults to dismissing. */
  dismissOnClick?: boolean;
}>;

export type AppToastOptions = Readonly<{
  id?: string;
  closeButton?: boolean;
  message: ReactNode;
  description?: ReactNode;
  tone?: AppToastTone;
  leading?: ReactNode;
  action?: AppToastAction;
  durationMs?: number;
  dismissible?: boolean;
  /** Runs once when the toast starts leaving, whatever dismissed it: the user, the timer, or code. */
  onDismiss?: (reason: "user" | "timer" | "programmatic") => void;
}>;

/** Fields a live toast can morph in place; the rest are fixed at show time. */
export type AppToastUpdate = Readonly<
  Partial<
    Pick<
      AppToastOptions,
      "message" | "description" | "tone" | "leading" | "action"
    >
  >
>;

export type AppToastHandle = Readonly<{
  /** Morph the visible toast without re-spawning it. A no-op once it is leaving. */
  update: (changes: AppToastUpdate) => void;
  dismiss: () => void;
}>;

export type AppToastAlign = "center" | "end";
/** `compact` matches desktop chrome: `size-8` icon buttons with `size-4` glyphs. */
export type AppToastSize = "default" | "compact";

export type AppToasterProps = Readonly<{
  theme?: "light" | "dark" | "system";
  position?: "top" | "bottom";
  /** Horizontal placement of the stack: centered (mobile) or flush right (desktop). */
  align?: AppToastAlign;
  size?: AppToastSize;
  visibleToasts?: number;
  gap?: number;
  className?: string;
  style?: CSSProperties;
}>;

type AppToastPhase = "visible" | "exiting";
type AppToastDirection = -1 | 1;

type AppToastItem = Readonly<{
  id: string;
  message: ReactNode;
  description?: ReactNode;
  tone: AppToastTone;
  leading?: ReactNode;
  action?: AppToastAction;
  durationMs: number;
  remainingMs: number;
  dismissible: boolean;
  closeButton: boolean;
  onDismiss?: (reason: "user" | "timer" | "programmatic") => void;
  paused: boolean;
  phase: AppToastPhase;
  exitDirection: AppToastDirection;
}>;

type AppToastTimer = Readonly<{
  deadline: number;
  timeoutId: ReturnType<typeof setTimeout>;
  tickId: ReturnType<typeof setInterval>;
}>;

const DEFAULT_APP_TOAST_DURATION_MS = 3_000;
const APP_TOAST_CORNER_RADIUS = 20;
const APP_TOAST_COMPACT_CORNER_RADIUS = 14;
const APP_TOAST_ICON_BUTTON_CLASS =
  "inline-flex shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none";
const APP_TOAST_EXIT_DURATION_MS = 220;
const APP_TOAST_TICK_MS = 100;
const APP_TOAST_DESKTOP_HOVER_QUERY = "(hover: hover) and (pointer: fine)";
const APP_TOAST_STACK_OFFSET_PX = 10;
const APP_TOAST_STACK_SCALE_STEP = 0.035;

let nextToastId = 0;
let appToastSnapshot: readonly AppToastItem[] = [];
const appToastListeners = new Set<() => void>();
const appToastTimers = new Map<string, AppToastTimer>();
const appToastExitTimers = new Map<string, ReturnType<typeof setTimeout>>();

function subscribeToAppToasts(listener: () => void): () => void {
  appToastListeners.add(listener);
  return () => appToastListeners.delete(listener);
}

function getAppToastSnapshot(): readonly AppToastItem[] {
  return appToastSnapshot;
}

function publishAppToasts(next: readonly AppToastItem[]): void {
  appToastSnapshot = next;
  appToastListeners.forEach((listener) => listener());
}

function updateAppToast(
  id: string,
  update: (toast: AppToastItem) => AppToastItem,
): void {
  let changed = false;
  const next = appToastSnapshot.map((toast) => {
    if (toast.id !== id) return toast;
    changed = true;
    return update(toast);
  });
  if (changed) publishAppToasts(next);
}

function clearAppToastTimer(id: string): void {
  const timer = appToastTimers.get(id);
  if (!timer) return;
  clearTimeout(timer.timeoutId);
  clearInterval(timer.tickId);
  appToastTimers.delete(id);
}

function removeAppToast(id: string): void {
  clearAppToastTimer(id);
  const exitTimer = appToastExitTimers.get(id);
  if (exitTimer) clearTimeout(exitTimer);
  appToastExitTimers.delete(id);
  publishAppToasts(appToastSnapshot.filter((toast) => toast.id !== id));
}

export function dismissAppToast(
  id: string,
  direction: AppToastDirection = 1,
  reason: "user" | "timer" | "programmatic" = "programmatic",
): void {
  const toast = appToastSnapshot.find((candidate) => candidate.id === id);
  if (!toast || toast.phase === "exiting") return;

  clearAppToastTimer(id);
  updateAppToast(id, (current) => ({
    ...current,
    phase: "exiting",
    exitDirection: direction,
  }));
  appToastExitTimers.set(
    id,
    setTimeout(() => removeAppToast(id), APP_TOAST_EXIT_DURATION_MS),
  );
  toast.onDismiss?.(reason);
}

function updateRemainingTime(id: string): void {
  const timer = appToastTimers.get(id);
  if (!timer) return;
  const remainingMs = Math.max(0, timer.deadline - Date.now());
  updateAppToast(id, (toast) =>
    toast.remainingMs === remainingMs ? toast : { ...toast, remainingMs },
  );
}

function startAppToastTimer(id: string, remainingMs: number): void {
  if (!Number.isFinite(remainingMs)) return;

  const safeRemainingMs = Math.max(0, remainingMs);
  const deadline = Date.now() + safeRemainingMs;
  const timeoutId = setTimeout(
    () => dismissAppToast(id, 1, "timer"),
    safeRemainingMs,
  );
  const tickId = setInterval(() => updateRemainingTime(id), APP_TOAST_TICK_MS);
  appToastTimers.set(id, { deadline, timeoutId, tickId });
}

function toggleAppToastTimer(id: string): void {
  const toast = appToastSnapshot.find((candidate) => candidate.id === id);
  if (!toast || !Number.isFinite(toast.durationMs) || toast.phase === "exiting")
    return;

  if (toast.paused) {
    updateAppToast(id, (current) => ({ ...current, paused: false }));
    startAppToastTimer(id, toast.remainingMs);
    return;
  }

  const timer = appToastTimers.get(id);
  const remainingMs = timer
    ? Math.max(0, timer.deadline - Date.now())
    : toast.remainingMs;
  clearAppToastTimer(id);
  updateAppToast(id, (current) => ({ ...current, paused: true, remainingMs }));
}

/**
 * App-wide toast viewport shared by desktop and web.
 * Mount once per renderer root; feedback is published through
 * `showAppToast` and friends.
 */
export function AppToaster({
  theme = "system",
  position = "bottom",
  align = "center",
  size = "default",
  visibleToasts = 4,
  gap = 8,
  className,
  style,
}: AppToasterProps) {
  const toasts = useSyncExternalStore(
    subscribeToAppToasts,
    getAppToastSnapshot,
    getAppToastSnapshot,
  );
  const [desktopStackExpanded, setDesktopStackExpanded] = useState(false);
  const visibleToastItems = toasts.slice(-visibleToasts);
  const supportsDesktopHover =
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia(APP_TOAST_DESKTOP_HOVER_QUERY).matches;
  const stacked =
    supportsDesktopHover &&
    visibleToastItems.length > 1 &&
    !desktopStackExpanded;

  const expandDesktopStack = () => {
    if (supportsDesktopHover) setDesktopStackExpanded(true);
  };
  const collapseDesktopStack = () => {
    if (supportsDesktopHover) setDesktopStackExpanded(false);
  };
  const handleStackBlur = (event: FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget))
      collapseDesktopStack();
  };

  if (typeof document === "undefined") return null;

  return createPortal(
    <div
      data-slot="app-toaster"
      data-app-toaster=""
      data-theme={theme}
      data-x-position={align}
      data-y-position={position}
      aria-live="polite"
      aria-relevant="additions"
      className={cn([
        "pointer-events-none fixed inset-x-0 isolate z-50 flex",
        align === "end"
          ? "justify-end pr-[max(1rem,env(safe-area-inset-right))]"
          : "justify-center",
        position === "top"
          ? "top-[max(1rem,env(safe-area-inset-top))] items-start"
          : "bottom-[max(1rem,env(safe-area-inset-bottom))] items-end",
        theme === "dark" ? "dark" : undefined,
        className,
      ])}
      style={style}
    >
      <motion.div
        layout
        data-slot="app-toast-stack"
        data-app-toast-stack=""
        data-hover-capable={supportsDesktopHover}
        data-stacked={stacked}
        onMouseEnter={expandDesktopStack}
        onMouseLeave={collapseDesktopStack}
        onFocusCapture={expandDesktopStack}
        onBlurCapture={handleStackBlur}
        className={cn([
          "pointer-events-auto relative w-fit",
          stacked ? "grid" : "flex flex-col",
        ])}
        style={{ gap }}
      >
        {visibleToastItems.map((toast, index) => {
          const stackDepth = visibleToastItems.length - index - 1;
          return (
            <AppToast
              key={toast.id}
              toast={toast}
              size={size}
              stackDepth={stackDepth}
              stacked={stacked}
            />
          );
        })}
      </motion.div>
    </div>,
    document.body,
  );
}

export function showAppToast({
  id = `app-toast-${++nextToastId}`,
  closeButton = true,
  message,
  description,
  tone = "info",
  leading,
  action,
  durationMs = DEFAULT_APP_TOAST_DURATION_MS,
  dismissible = true,
  onDismiss,
}: AppToastOptions): AppToastHandle {
  clearAppToastTimer(id);
  const exitTimer = appToastExitTimers.get(id);
  if (exitTimer) clearTimeout(exitTimer);
  appToastExitTimers.delete(id);
  const normalizedDurationMs = Number.isFinite(durationMs)
    ? Math.max(0, durationMs)
    : durationMs;
  const toast: AppToastItem = {
    id,
    message,
    description,
    tone,
    leading,
    action,
    durationMs: normalizedDurationMs,
    remainingMs: normalizedDurationMs,
    dismissible,
    closeButton,
    onDismiss,
    paused: false,
    phase: "visible",
    exitDirection: 1,
  };

  publishAppToasts(
    appToastSnapshot.some((item) => item.id === id)
      ? appToastSnapshot.map((item) => (item.id === id ? toast : item))
      : [...appToastSnapshot, toast],
  );
  startAppToastTimer(id, normalizedDurationMs);

  return {
    update: (changes) =>
      updateAppToast(id, (current) =>
        current.phase === "exiting" ? current : { ...current, ...changes },
      ),
    dismiss: () => dismissAppToast(id),
  };
}

export function showErrorToast(message: string): AppToastHandle {
  return showAppToast({ message, tone: "error" });
}

export function showSuccessToast(message: string): AppToastHandle {
  return showAppToast({ message, tone: "success" });
}

export function dismissAppToasts(): void {
  const leaving = appToastSnapshot.filter((toast) => toast.phase === "visible");
  appToastTimers.forEach((_, id) => clearAppToastTimer(id));
  appToastExitTimers.forEach((timer) => clearTimeout(timer));
  appToastExitTimers.clear();
  publishAppToasts([]);
  leaving.forEach((toast) => toast.onDismiss?.("programmatic"));
}

type AppToastProps = Readonly<{
  toast: AppToastItem;
  size: AppToastSize;
  stackDepth: number;
  stacked: boolean;
}>;

function AppToast({ toast, size, stackDepth, stacked }: AppToastProps) {
  const compact = size === "compact";
  const squircleRef = useSquircleRef<HTMLDivElement>(undefined, {
    ...panelSquircle,
    radius: compact ? APP_TOAST_COMPACT_CORNER_RADIUS : APP_TOAST_CORNER_RADIUS,
  });
  const reduceMotion = useReducedMotion() ?? false;
  const [expanded, setExpanded] = useState(true);
  const detailsId = useId();
  const hasTimer = Number.isFinite(toast.durationMs);
  const hasDetails =
    hasTimer || toast.description !== undefined || toast.action !== undefined;
  const stackedBehind = stacked && stackDepth > 0;
  const visuallyExpanded = expanded && !stackedBehind;
  const stackScale = Math.max(
    0.88,
    1 - stackDepth * APP_TOAST_STACK_SCALE_STEP,
  );
  const exitDistance =
    typeof window === "undefined" ? 1_024 : window.innerWidth + 256;
  const progress =
    hasTimer && toast.durationMs > 0
      ? Math.max(0, Math.min(1, toast.remainingMs / toast.durationMs))
      : 0;

  const dismiss = (direction: AppToastDirection = 1) =>
    dismissAppToast(toast.id, direction, "user");
  const handleCardClick = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!toast.dismissible || event.defaultPrevented) return;
    const target = event.target as HTMLElement;
    if (target.closest?.("button, a, input, textarea, select, [role=button]"))
      return;
    dismiss();
  };
  const handleDragEnd = (
    _event: globalThis.MouseEvent | TouchEvent | PointerEvent,
    info: PanInfo,
  ) => {
    if (!toast.dismissible) return;
    const direction = appToastSwipeDismissDirection(
      info.offset.x,
      info.velocity.x,
    );
    if (direction !== undefined) dismiss(direction);
  };

  return (
    <motion.div
      layout
      role={toast.tone === "error" ? "alert" : "status"}
      data-slot="app-toast"
      data-app-toast=""
      data-tone={toast.tone}
      data-size={size}
      data-expanded={visuallyExpanded}
      data-phase={toast.phase}
      data-exit-direction={toast.exitDirection}
      data-stack-depth={stackDepth}
      data-stacked-behind={stackedBehind}
      inert={stackedBehind}
      initial={reduceMotion ? false : { opacity: 0, y: 20, scale: 0.96 }}
      animate={
        toast.phase === "exiting"
          ? {
              opacity: 0,
              x: toast.exitDirection * exitDistance,
              scale: 0.96,
            }
          : {
              opacity: 1,
              x: 0,
              y: stacked ? -stackDepth * APP_TOAST_STACK_OFFSET_PX : 0,
              scale: stacked ? stackScale : 1,
            }
      }
      transition={
        reduceMotion
          ? { duration: 0 }
          : {
              layout: { duration: 0.28, ease: [0.22, 1, 0.36, 1] },
              opacity: { duration: 0.18, ease: "easeOut" },
              x: {
                duration: APP_TOAST_EXIT_DURATION_MS / 1_000,
                ease: [0.4, 0, 1, 1],
              },
              y: { duration: 0.24, ease: [0.22, 1, 0.36, 1] },
              scale: { duration: 0.2, ease: "easeOut" },
            }
      }
      drag={toast.dismissible && toast.phase === "visible" ? "x" : false}
      dragConstraints={{ left: 0, right: 0 }}
      dragElastic={0.7}
      dragMomentum={false}
      onDragEnd={handleDragEnd}
      onClick={handleCardClick}
      whileDrag={reduceMotion ? undefined : { scale: 0.985 }}
      className={cn([
        "no-drag pointer-events-auto col-start-1 row-start-1 mx-auto origin-top touch-pan-y self-start select-none",
        compact
          ? "w-[min(20rem,calc(100vw-2rem))] rounded-[14px] shadow-md"
          : "w-[min(32rem,calc(100vw-2rem))] rounded-[20px] shadow-lg",
        toast.dismissible && toast.phase === "visible"
          ? "cursor-pointer active:cursor-grabbing"
          : null,
        toast.phase === "exiting" || stackedBehind
          ? "pointer-events-none"
          : null,
      ])}
      style={{ zIndex: stackDepth === 0 ? 10 : 10 - stackDepth }}
    >
      <div
        ref={squircleRef}
        data-slot="smooth-corners"
        className="bg-popover text-popover-foreground ring-border/70 relative ring-1"
      >
        <div
          className={cn([
            "flex items-center",
            compact
              ? "min-h-11 gap-2 px-2.5 py-1.5"
              : "min-h-16 gap-3 px-4 py-3",
          ])}
        >
          {/* The slot sizes the glyph, so callers only pass color. */}
          <span
            data-slot="app-toast-leading"
            className={cn([
              "flex shrink-0 items-center justify-center",
              compact ? "size-8 [&>svg]:size-4" : "size-7 [&>svg]:size-6",
            ])}
            aria-hidden
          >
            {toast.leading ?? <AppToastToneIcon tone={toast.tone} />}
          </span>
          <span
            data-slot="app-toast-message"
            className={cn([
              "min-w-0 flex-1 font-semibold break-words whitespace-normal",
              compact ? "text-[13px] leading-5" : "text-sm",
            ])}
          >
            {toast.message}
          </span>
          {hasDetails ? (
            <button
              type="button"
              aria-label={
                visuallyExpanded
                  ? "Collapse notification"
                  : "Expand notification"
              }
              aria-expanded={visuallyExpanded}
              aria-controls={detailsId}
              onClick={() => setExpanded((current) => !current)}
              className={cn([
                APP_TOAST_ICON_BUTTON_CLASS,
                compact ? "size-8" : "size-9",
              ])}
            >
              <motion.span
                animate={{ rotate: visuallyExpanded ? 180 : 0 }}
                transition={
                  reduceMotion
                    ? { duration: 0 }
                    : { duration: 0.2, ease: "easeOut" }
                }
              >
                <CaretDown className="size-4" aria-hidden />
              </motion.span>
            </button>
          ) : null}
          {toast.closeButton && toast.dismissible ? (
            <button
              type="button"
              aria-label="Dismiss"
              onClick={() => dismiss()}
              className={cn([
                APP_TOAST_ICON_BUTTON_CLASS,
                compact ? "size-8" : "size-9",
              ])}
            >
              <X className="size-4" aria-hidden />
            </button>
          ) : null}
        </div>

        <AnimatePresence initial={false}>
          {visuallyExpanded && hasDetails ? (
            <motion.div
              id={detailsId}
              key="details"
              data-slot="app-toast-details"
              initial={reduceMotion ? false : { height: 0, opacity: 0 }}
              animate={{ height: "auto", opacity: 1 }}
              exit={{ height: 0, opacity: 0 }}
              transition={
                reduceMotion
                  ? { duration: 0 }
                  : {
                      height: { duration: 0.28, ease: [0.22, 1, 0.36, 1] },
                      opacity: { duration: 0.16, ease: "easeOut" },
                    }
              }
              className="overflow-hidden"
            >
              {toast.description !== undefined || toast.action !== undefined ? (
                <div
                  className={
                    compact ? "space-y-2 px-2.5 pb-2.5" : "space-y-3 px-4 pb-3"
                  }
                >
                  {toast.description !== undefined ? (
                    <div
                      className={cn([
                        "text-muted-foreground",
                        compact ? "text-xs leading-4" : "text-sm leading-5",
                      ])}
                    >
                      {toast.description}
                    </div>
                  ) : null}
                  {toast.action ? (
                    <button
                      type="button"
                      onClick={(event) => {
                        try {
                          toast.action?.onClick(event);
                        } finally {
                          if (
                            !event.defaultPrevented &&
                            toast.action?.dismissOnClick !== false
                          )
                            dismiss();
                        }
                      }}
                      className={cn([
                        "border-border bg-background text-foreground hover:bg-accent focus-visible:ring-ring inline-flex items-center justify-center border font-medium transition-[color,background-color,transform] focus-visible:ring-2 focus-visible:outline-none active:scale-[0.98]",
                        compact
                          ? "min-h-7 rounded-md px-2.5 text-xs"
                          : "min-h-9 rounded-lg px-3 text-sm",
                      ])}
                    >
                      {toast.action.label}
                    </button>
                  ) : null}
                </div>
              ) : null}

              {hasTimer ? (
                <button
                  type="button"
                  aria-label={
                    toast.paused ? "Resume auto-dismiss" : "Pause auto-dismiss"
                  }
                  onClick={() => toggleAppToastTimer(toast.id)}
                  className={cn([
                    "border-border/60 bg-muted/50 text-muted-foreground hover:bg-muted focus-visible:ring-ring relative flex w-full items-center justify-between gap-3 border-t text-left transition-colors focus-visible:ring-2 focus-visible:outline-none focus-visible:ring-inset",
                    compact
                      ? "min-h-8 px-2.5 py-1.5 text-[11px]"
                      : "min-h-10 px-4 py-2 text-xs",
                  ])}
                >
                  <span aria-hidden>
                    {toast.paused ? "Timer paused" : "Closing in"}{" "}
                    <span className="text-foreground font-semibold tabular-nums">
                      {Math.max(0, Math.ceil(toast.remainingMs / 1_000))}
                    </span>{" "}
                    {Math.ceil(toast.remainingMs / 1_000) === 1
                      ? "second"
                      : "seconds"}
                  </span>
                  <span aria-hidden className="text-foreground font-medium">
                    {toast.paused ? "Resume" : "Pause"}
                  </span>
                  <motion.span
                    aria-hidden
                    data-slot="app-toast-progress"
                    className={cn([
                      "absolute inset-x-0 bottom-0 h-0.5 origin-left",
                      toast.tone === "error"
                        ? "bg-destructive"
                        : toast.tone === "success"
                          ? "bg-emerald-500"
                          : "bg-blue-500",
                    ])}
                    initial={false}
                    animate={{ scaleX: progress }}
                    transition={
                      reduceMotion
                        ? { duration: 0 }
                        : { duration: 0.1, ease: "linear" }
                    }
                  />
                </button>
              ) : null}
            </motion.div>
          ) : null}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}

function AppToastToneIcon({ tone }: Readonly<{ tone: AppToastTone }>) {
  switch (tone) {
    case "success":
      return <CheckCircle className="text-emerald-600" />;
    case "warning":
      return <WarningCircle className="text-amber-600" />;
    case "error":
      return <WarningCircle className="text-destructive" />;
    case "info":
      return <Info className="text-blue-600" />;
  }
}
