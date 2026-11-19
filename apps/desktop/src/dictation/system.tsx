import { platform } from "@tauri-apps/plugin-os";
import { useRef } from "react";

import {
  commands as dictation,
  type ContextExclusions,
  type InsertRequest,
  type TargetContext,
} from "@anlg/plugin-dictation";
import { commands as permissions } from "@anlg/plugin-permissions";
import { commands as shortcuts, events } from "@anlg/plugin-shortcut";
import { commands as transcription } from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import {
  DictationEngine,
  DictationRetired,
  type DictationView,
} from "./engine";
import { nativeDictation } from "./native";
import { waitForDictationPanel } from "./panel";
import { useDictationPlanner } from "./planner";
import { serialDictation } from "./serial";
import { useDictationStatus } from "./state";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import { useConfigValue } from "~/shared/config";

// One read of the field this long after an insert tells what the person kept of it.
const KEPT_READ_MS = 20_000;

export const GESTURES = {
  holdMs: 250,
  doubleTapMs: 400,
  doubleTapLatch: true,
  spaceLatch: true,
};

function unwrap<T>(
  result: { status: "ok"; data: T } | { status: "error"; error: string },
): T {
  if (result.status === "error") throw new Error(result.error);
  return result.data;
}

export async function checkDictationReady() {
  if (platform() === "macos") {
    const permission = unwrap(await permissions.checkPermission("microphone"));
    if (permission !== "authorized")
      throw new Error(
        "Enable microphone permission in Settings > Permissions before dictating.",
      );
  }
  const capture = unwrap(await transcription.getCaptureState());
  if (capture !== "inactive")
    throw new Error(
      "Dictation is unavailable while Anarlog is recording a meeting.",
    );
}

export function parseContextExclusions(
  value: string | undefined,
): ContextExclusions | null {
  if (!value) return { apps: [], websites: [] };
  try {
    return JSON.parse(value) as ContextExclusions;
  } catch {
    // Unreadable exclusions deny external context natively.
    return null;
  }
}

/** System-wide dictation into the field focused when it starts. */
export function SystemDictation({
  shortcut,
  handsFree,
}: {
  shortcut: string;
  handsFree: boolean;
}) {
  const id = useRef(`system-dictation-${crypto.randomUUID()}`).current;
  const planner = useDictationPlanner(id);
  const auth = useAuth();
  const exclusions = useConfigValue("dictation_context_exclusions");
  const current = useRef({
    account: auth.session?.user.id ?? null,
    exclusions,
  });
  current.current = { account: auth.session?.user.id ?? null, exclusions };

  useMountEffect(() => {
    let disposed = false;
    let armed = false;
    let unlisten: (() => void) | undefined;
    let presentation: Promise<void> = Promise.resolve();
    let active = false;
    const onError = (message: string) => {
      if (disposed) return;
      useDictationStatus.setState({ error: message });
      toast.error(message);
    };
    const setActive = (next: boolean) => {
      if (next === active) return;
      active = next;
      presentation = presentation
        .then(async () => {
          unwrap(await shortcuts.setActive(next && !disposed));
        })
        .catch((error: unknown) =>
          onError(error instanceof Error ? error.message : String(error)),
        );
    };
    const onView = (view: DictationView | null) => {
      if (disposed) return;
      if (!view) {
        useDictationStatus.setState({
          phase: "idle",
          owner: null,
          presentedOwner: null,
          text: "",
          partial: "",
          amplitude: 0,
        });
        setActive(false);
        return;
      }
      const owner = `${id}:${view.operationId}`;
      const state = useDictationStatus.getState();
      useDictationStatus.setState({
        owner,
        phase: view.phase,
        amplitude: view.phase === "recording" ? view.amplitude : 0,
        text: view.text,
        partial: view.partial,
        previewEnabled: view.livePreview,
        previewUnavailable: view.previewUnavailable,
        ...(state.owner !== owner
          ? {
              error: null,
              microphone: planner.microphone() || "Default microphone",
            }
          : {}),
        // The plan, read after the press, says whether live words show: open the panel then.
        ...(state.owner !== owner || state.previewEnabled !== view.livePreview
          ? { expanded: view.livePreview }
          : {}),
      });
      setActive(true);
    };

    const engine = new DictationEngine({
      surface: "system",
      native: nativeDictation,
      account: () => current.current.account,
      prepare: checkDictationReady,
      open: async (operationId, signal) => {
        const target = unwrap(await dictation.captureTarget());
        // Ends the field natively, also for a write into it still waiting there.
        const release = async () => {
          try {
            await dictation.releaseDictationTarget(target);
          } catch {
            // Native still limits the field to one delivery and the window after it.
          }
        };
        try {
          signal.throwIfAborted();
          const options = planner.options();
          const exclusions = parseContextExclusions(current.current.exclusions);
          let context: TargetContext | null = null;
          if (options.context || options.voiceEdit) {
            // Native decides privacy first; a denied or failed read leaves plain dictation.
            const read = await dictation.readDictationContext(target, {
              operationId,
              external: options.context,
              selection: options.voiceEdit,
              exclusions,
            });
            context = read.status === "ok" ? read.data : null;
          }
          signal.throwIfAborted();
          const accountId = current.current.account;
          if (!accountId) throw new DictationRetired();
          const plan = await planner.plan({
            accountId,
            context: options.context ? (context?.snapshot ?? null) : null,
          });
          const selection =
            options.voiceEdit && context?.fingerprint
              ? context.selection
              : null;
          let applied = false;
          return {
            plan,
            target: {
              selection,
              before: null,
              context: options.context ? (context?.snapshot ?? null) : null,
              deliver: async (text, edit, isCurrent) => {
                if (!isCurrent()) return "unchanged";
                const mode: InsertRequest["mode"] =
                  edit && context?.fingerprint
                    ? {
                        type: "replaceSelection",
                        fingerprint: context.fingerprint,
                      }
                    : { type: "fit", canJoin: true };
                // Native writes only while this operation can still deliver.
                const outcome = unwrap(
                  await dictation.insertDictation(target, {
                    operationId,
                    editId: operationId,
                    text,
                    mode,
                    exclusions,
                  }),
                );
                if (outcome.type !== "applied") return outcome.type;
                applied = true;
                if (isCurrent())
                  void planner.used(accountId, text).catch(() => {});
                return "applied";
              },
              learn: options.learning
                ? async (inserted, isCurrent, signal) => {
                    await abortableDelay(KEPT_READ_MS, signal);
                    if (!isCurrent()) return;
                    // The token of this insert, even if a later press captured another field.
                    const kept = unwrap(
                      await dictation.readDictationKept(
                        target,
                        {
                          operationId,
                          external: false,
                          selection: false,
                          exclusions,
                        },
                        operationId,
                      ),
                    );
                    if (kept)
                      await planner.kept(accountId, inserted, kept, isCurrent);
                  }
                : undefined,
              restoreRaw: async (raw) => {
                if (!applied) return;
                // Native checks the same focused field still ends with what it typed.
                unwrap(
                  await dictation.insertDictation(target, {
                    operationId,
                    editId: `${operationId}:raw`,
                    text: raw,
                    mode: { type: "restore", editId: operationId },
                    exclusions,
                  }),
                );
              },
              release,
            },
          };
        } catch (error) {
          void release();
          throw error;
        }
      },
      present: async (operationId, signal) => {
        await waitForDictationPanel(`${id}:${operationId}`, signal);
      },
      onView,
      onError,
      onTranscript: (lastTranscript) => {
        if (!disposed) useDictationStatus.setState({ lastTranscript });
      },
      onMeasured: (metrics) =>
        trackAnalyticsEvent("dictation_completed", {
          surface: "system",
          source: metrics.source,
          refined: metrics.refined,
          delivery: metrics.delivery,
          stop_to_text_ms: metrics.stopToTextMs,
          transcript_ms: metrics.transcriptMs,
          refine_ms: metrics.refineMs,
          insert_ms: metrics.insertMs,
        }),
      onRecoveryKept: () =>
        useDictationStatus.setState((state) => ({
          recoveryVersion: state.recoveryVersion + 1,
        })),
    });

    void serialDictation(async () => {
      if (disposed) return;
      if (handsFree) {
        // Hands-free keeps its press-to-start, press-to-finish shortcut.
        unlisten = await events.shortcutEvent.listen(({ payload }) => {
          if (
            disposed ||
            !armed ||
            useDictationStatus.getState().capturingShortcut
          )
            return;
          if (payload.type === "pressed") engine.toggle();
          else if (payload.type === "cancelled") engine.cancel();
        });
        if (!disposed) unwrap(await shortcuts.configure(shortcut));
      } else {
        unlisten = await events.dictationGesture.listen(({ payload }) => {
          if (
            disposed ||
            !armed ||
            useDictationStatus.getState().capturingShortcut
          )
            return;
          switch (payload.type) {
            case "prime":
              return engine.prime();
            case "unprime":
              return engine.unprime();
            case "start":
              return engine.start(payload.latched);
            case "latch":
              return engine.latch();
            case "stop":
              return engine.stop();
            case "cancel":
              return engine.cancel();
            case "tap":
              return engine.tap();
          }
        });
        if (!disposed)
          unwrap(await shortcuts.configureGestures(shortcut, GESTURES));
      }
      if (disposed) {
        unlisten?.();
        return;
      }
      armed = true;
      useDictationStatus.setState({
        ready: true,
        error: null,
        cancel: () => engine.cancel(),
        finish: () => engine.stop(),
      });
    }).catch((error: unknown) =>
      onError(error instanceof Error ? error.message : String(error)),
    );

    return () => {
      disposed = true;
      armed = false;
      unlisten?.();
      useDictationStatus.setState({
        ready: false,
        phase: "idle",
        cancel: null,
        finish: null,
        owner: null,
        presentedOwner: null,
        text: "",
        partial: "",
      });
      void serialDictation(async () => {
        unlisten?.();
        await engine.dispose();
        await presentation;
        unwrap(await shortcuts.setActive(false));
        unwrap(
          handsFree
            ? await shortcuts.configure(null)
            : await shortcuts.configureGestures(null, GESTURES),
        );
      }).catch(() => {});
    };
  });
  return null;
}

function abortableDelay(ms: number, signal: AbortSignal) {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) return reject(new DictationRetired());
    const timer = setTimeout(resolve, ms);
    signal.addEventListener(
      "abort",
      () => {
        clearTimeout(timer);
        reject(new DictationRetired());
      },
      { once: true },
    );
  });
}
