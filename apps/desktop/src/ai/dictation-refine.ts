import { generateText, type LanguageModel } from "ai";

import {
  commands,
  type ModelReply,
  type PendingRefinement,
  type RefineOutcome,
  type RefineRequest,
} from "@anlg/plugin-dictation";

/**
 * Cleans up a finished dictation, or edits the selection by voice, through the one model the
 * person selected (`useLanguageModel()`, with its own credentials and auth fetch). The rules for
 * whether a call happens, what it says, its deadline and which reply may be typed live in
 * `dictation-core`; this only makes the call. It always resolves: a failure types the raw words
 * for cleanup and nothing for an edit, and there is never a second route. Cancellation wins at
 * once, even while the rules or the model are still answering.
 */
export async function refineDictation({
  model,
  request,
  signal,
}: {
  model: LanguageModel | null;
  request: RefineRequest;
  signal?: AbortSignal;
}): Promise<RefineOutcome> {
  if (signal?.aborted) return { type: "cancelled" };
  // `refine` never rejects, so only the cancellation lands in `catch`.
  return until(refine(model, request, signal), signal).catch(
    (): RefineOutcome => ({ type: "cancelled" }),
  );
}

async function refine(
  model: LanguageModel | null,
  request: RefineRequest,
  signal: AbortSignal | undefined,
): Promise<RefineOutcome> {
  const unreachable = fallbackFor(request);
  let plan;
  try {
    plan = await commands.prepareDictationRefinement({
      ...request,
      modelReady: request.modelReady && model !== null,
    });
  } catch {
    return unreachable;
  }
  if (signal?.aborted) return { type: "cancelled" };
  if (plan.type === "done") return plan.outcome;
  if (!model) return unreachable;

  const reply = await ask(model, plan.call, signal);
  return resolve(plan.pending, reply, unreachable);
}

async function ask(
  model: LanguageModel,
  call: { system: string; prompt: string; deadlineMs: number },
  signal: AbortSignal | undefined,
): Promise<ModelReply> {
  if (signal?.aborted) return { type: "cancelled" };
  const deadline = new AbortController();
  const timer = setTimeout(() => deadline.abort(), call.deadlineMs);
  const stop = signal
    ? AbortSignal.any([signal, deadline.signal])
    : deadline.signal;
  let reply: ModelReply;
  try {
    // A provider may keep going after it's told to stop (a native model can wait on its own
    // cancellation), so the deadline doesn't wait for it to agree.
    const result = await until(
      generateText({
        model,
        system: call.system,
        prompt: call.prompt,
        // A retry would run past the deadline that decides between this and the raw words.
        maxRetries: 0,
        abortSignal: stop,
      }),
      stop,
    );
    reply = { type: "text", text: result.text };
  } catch {
    reply = { type: "failed" };
  } finally {
    clearTimeout(timer);
  }
  // An answer that arrives once the call was stopped no longer counts.
  if (signal?.aborted) return { type: "cancelled" };
  if (deadline.signal.aborted) return { type: "timedOut" };
  return reply;
}

async function resolve(
  pending: PendingRefinement,
  reply: ModelReply,
  unreachable: RefineOutcome,
): Promise<RefineOutcome> {
  if (reply.type === "cancelled") return { type: "cancelled" };
  try {
    return await commands.resolveDictationRefinement(pending, reply);
  } catch {
    return unreachable;
  }
}

/**
 * Settles with `work`, or rejects as soon as `signal` aborts. Whatever `work` does later is
 * dropped, and the listener goes with whichever comes first.
 */
function until<T>(
  work: Promise<T>,
  signal: AbortSignal | undefined,
): Promise<T> {
  if (!signal) return work;
  return new Promise<T>((settle, fail) => {
    const stop = () => fail(signal.reason);
    signal.addEventListener("abort", stop, { once: true });
    void work
      .then(settle, fail)
      .finally(() => signal.removeEventListener("abort", stop));
    if (signal.aborted) stop();
  });
}

/**
 * What to type when the rules themselves can't be reached: the words as heard (without saved
 * replacements, which need the rules), and never anything over a selection.
 */
function fallbackFor(request: RefineRequest): RefineOutcome {
  const text = request.text.trim();
  if (request.selection) return { type: "editFailed", reason: "failed" };
  if (!text) return { type: "empty" };
  return { type: "raw", text, reason: "failed" };
}
