import { useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";
import { useRef, useState } from "react";

import type { ChatEditorHandle, JSONContent } from "@anlg/editor/chat";
import { commands as dictationCommands } from "@anlg/plugin-dictation";
import { commands as permissionCommands } from "@anlg/plugin-permissions";
import { commands as transcriptionCommands } from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { trackAnalyticsEvent } from "~/analytics";
import { useAuth } from "~/auth";
import {
  DictationEngine,
  DictationRetired,
  NO_SPEECH_MESSAGE,
  type DictationView,
  type EnginePhase,
} from "~/dictation/engine";
import { keptBetween } from "~/dictation/learning";
import { nativeDictation } from "~/dictation/native";
import { useDictationPlanner } from "~/dictation/planner";
import { useDictationStatus } from "~/dictation/state";
import { useTabs } from "~/store/zustand/tabs";

const MAX_DICTATION_SECONDS = 5 * 60;

// One read of the composer this long after an insert tells what the person kept of it.
const KEPT_READ_MS = 20_000;
const MEETING_ACTIVE_MESSAGE =
  "Voice input is unavailable while Anarlog is recording a meeting.";

/** Voice input into the chat composer, at the caret or over the selection it started on. */
export function useDictation({
  editorRef,
  disabled,
}: {
  editorRef: React.RefObject<ChatEditorHandle | null>;
  disabled?: boolean;
}) {
  const { t } = useLingui();
  const transcriptionSessionId = useRef(
    `chat-dictation-${crypto.randomUUID()}`,
  ).current;
  const planner = useDictationPlanner(transcriptionSessionId);
  const auth = useAuth();
  const account = useRef<string | null>(null);
  account.current = auth.session?.user.id ?? null;
  const [view, setView] = useState<DictationView | null>(null);
  const phaseRef = useRef<EnginePhase>("idle");
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const engineRef = useRef<DictationEngine | null>(null);
  const openRef = useRef(true);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const stopTimer = () => {
    if (timerRef.current) clearInterval(timerRef.current);
    timerRef.current = null;
  };

  const engine = () =>
    (engineRef.current ??= new DictationEngine({
      surface: "chat",
      native: nativeDictation,
      account: () => account.current,
      prepare: prepareChatDictation,
      open: async () => {
        const accountId = account.current;
        if (!accountId) throw new DictationRetired();
        const options = planner.options();
        // Where the dictation started: a voice edit rewrites only this exact selection.
        const started = editorRef.current?.readSelection() ?? null;
        const selection = options.voiceEdit
          ? (started?.selected ?? null)
          : null;
        const plan = await planner.plan({ accountId, context: null });
        let surrounding: { before: string; after: string } | null = null;
        // `type` checks and writes in one synchronous transaction; nothing lands in between.
        const write = (type: () => boolean) => {
          const editor = editorRef.current;
          if (!editor) return false;
          const before = plainText(editor.getJSON());
          if (!type()) return false;
          surrounding = surroundingText(before, plainText(editor.getJSON()));
          editor.focus();
          return true;
        };
        return {
          plan,
          target: {
            selection,
            // The composer's text reaches the model only with on-screen context allowed.
            before: options.context ? started?.before || null : null,
            context: null,
            deliver: async (text, edit, isCurrent) => {
              const live = () => isCurrent() && openRef.current;
              const typed = edit
                ? live() && !!started && write(() => started.replace(text))
                : await typeAtCaret(editorRef, text, live, write);
              if (!typed) return "unchanged";
              void planner.used(accountId, text).catch(() => {});
              return "applied";
            },
            learn: options.learning
              ? async (inserted, isCurrent, signal) => {
                  await abortableDelay(KEPT_READ_MS, signal);
                  const editor = editorRef.current;
                  if (!isCurrent() || !editor || !surrounding) return;
                  const kept = keptBetween(
                    surrounding.before,
                    surrounding.after,
                    plainText(editor.getJSON()),
                  );
                  if (kept)
                    await planner.kept(accountId, inserted, kept, isCurrent);
                }
              : undefined,
          },
        };
      },
      onView: (next) => {
        phaseRef.current = next?.phase ?? "idle";
        setView(next);
        if (next?.phase === "recording" && !timerRef.current) {
          const startedAt = Date.now();
          setElapsedSeconds(0);
          timerRef.current = setInterval(() => {
            setElapsedSeconds(
              Math.min(
                Math.floor((Date.now() - startedAt) / 1_000),
                MAX_DICTATION_SECONDS,
              ),
            );
          }, 250);
        } else if (next?.phase !== "recording") {
          stopTimer();
        }
        if (!next) editorRef.current?.focus();
      },
      onError: (message) => {
        if (message === MEETING_ACTIVE_MESSAGE) {
          toast.warning(
            t`Voice input is unavailable while Anarlog is recording a meeting.`,
          );
          return;
        }
        if (message === NO_SPEECH_MESSAGE) {
          toast.warning(t`No speech detected`, {
            description: t`Try speaking a little closer to the microphone.`,
          });
          return;
        }
        // Nothing was transcribed yet: the microphone or the route never got going.
        if (phaseRef.current !== "transcribing") {
          toast.error(t`Could not start voice input`, { description: message });
          return;
        }
        toast.error(t`Could not transcribe voice input`, {
          description: message,
        });
      },
      onMeasured: (metrics) =>
        trackAnalyticsEvent("dictation_completed", {
          surface: "chat",
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
    }));

  useMountEffect(() => {
    openRef.current = useTabs.getState().chatMode !== "FloatingClosed";
    const unsubscribe = useTabs.subscribe((state, previous) => {
      if (state.chatMode === previous.chatMode) return;
      openRef.current = state.chatMode !== "FloatingClosed";
      // Closing chat retires its dictation; nothing late is inserted or uploaded.
      if (!openRef.current) engineRef.current?.cancel();
    });
    return () => {
      unsubscribe();
      openRef.current = false;
      stopTimer();
      const current = engineRef.current;
      engineRef.current = null;
      void current?.dispose();
    };
  });

  return {
    elapsedSeconds,
    phase: view?.phase ?? ("idle" as EnginePhase),
    start: async () => {
      if (disabled || !openRef.current) return;
      engine().start(true);
    },
    stop: async () => {
      engineRef.current?.stop();
    },
  };
}

async function prepareChatDictation() {
  if (platform() === "macos") {
    // A click on the microphone button is an explicit request, so it may ask for access.
    let status = await permissionCommands.checkPermission("microphone");
    if (status.status === "ok" && status.data === "neverRequested") {
      await permissionCommands.requestPermission("microphone");
      status = await permissionCommands.checkPermission("microphone");
    }
    if (status.status === "error") throw new Error(status.error);
    if (status.data !== "authorized")
      throw new Error(
        "Allow microphone access in System Settings > Privacy & Security > Microphone.",
      );
  }
  const capture = await transcriptionCommands.getCaptureState();
  if (capture.status === "error") throw new Error(capture.error);
  if (capture.data !== "inactive") throw new Error(MEETING_ACTIVE_MESSAGE);
}

// Typing while a fit is computed moves the caret, so the text is fitted again to where it is.
const FIT_ATTEMPTS = 3;

/** Types a plain dictation at the caret as it is now, fitted to the text before it. */
async function typeAtCaret(
  editorRef: React.RefObject<ChatEditorHandle | null>,
  text: string,
  live: () => boolean,
  write: (type: () => boolean) => boolean,
) {
  for (let attempt = 0; attempt < FIT_ATTEMPTS; attempt++) {
    const read = editorRef.current?.readSelection();
    if (!read || !live()) return false;
    let fit = { text, join: false };
    try {
      fit = await dictationCommands.fitDictationText(
        text,
        read.before,
        read.empty,
      );
    } catch {
      // Fitting is a nicety; the transcript itself is still typed.
    }
    if (!live()) return false;
    if (
      write(() =>
        read.replace(fit.text, {
          join: fit.join,
          separateFollowingWord: true,
        }),
      )
    )
      return true;
  }
  return false;
}

function plainText(json: JSONContent | undefined): string {
  if (!json) return "";
  if (typeof json.text === "string") return json.text;
  const children = (json.content ?? []).map(plainText);
  return json.type === "doc" ? children.join("\n") : children.join("");
}

function surroundingText(before: string, after: string) {
  let start = 0;
  while (start < before.length && before[start] === after[start]) start++;
  let end = 0;
  while (
    end < before.length - start &&
    before[before.length - 1 - end] === after[after.length - 1 - end]
  )
    end++;
  return {
    before: before.slice(0, start),
    after: before.slice(before.length - end),
  };
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
