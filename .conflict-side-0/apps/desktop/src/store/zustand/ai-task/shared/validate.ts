import type { TextStreamPart, ToolSet } from "ai";

export type EarlyValidatorFn = (
  textSoFar: string,
) => { valid: true } | { valid: false; feedback: string };

export async function* withEarlyValidationRetry<
  TOOLS extends ToolSet = ToolSet,
>(
  executeStream: (
    signal: AbortSignal,
    attemptContext: { attempt: number; previousFeedback?: string },
  ) => AsyncIterable<TextStreamPart<TOOLS>>,
  validator: EarlyValidatorFn,
  options: {
    minChar?: number;
    maxChar?: number;
    maxRetries?: number;
    onRetry?: (attempt: number, feedback: string) => void;
    onRetrySuccess?: () => void;
    onGiveUp?: () => void;
  } = {},
): AsyncIterable<TextStreamPart<TOOLS>> {
  const {
    minChar = 5,
    maxChar = 30,
    maxRetries = 2,
    onRetry,
    onRetrySuccess,
    onGiveUp,
  } = options;

  let previousFeedback: string | undefined;

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    const abortController = new AbortController();
    const buffer: TextStreamPart<TOOLS>[] = [];
    let accumulatedText = "";
    let validationComplete = false;

    const flushBuffer = function* () {
      validationComplete = true;
      if (attempt > 0) {
        onRetrySuccess?.();
      }
      yield* buffer;
      buffer.length = 0;
    };

    try {
      const stream = executeStream(abortController.signal, {
        attempt,
        previousFeedback,
      });

      for await (const chunk of stream) {
        if (!validationComplete) {
          buffer.push(chunk);

          if (chunk.type === "text-delta") {
            accumulatedText += chunk.text;
            const trimmedLength = accumulatedText.trim().length;

            if (trimmedLength >= minChar) {
              const result = validator(accumulatedText);

              if (!result.valid) {
                if (attempt < maxRetries - 1) {
                  abortController.abort();
                  previousFeedback = result.feedback;
                  onRetry?.(attempt + 1, result.feedback);
                  break;
                }

                onGiveUp?.();
                yield* flushBuffer();
                continue;
              }

              yield* flushBuffer();
            } else if (accumulatedText.length >= maxChar) {
              yield* flushBuffer();
            }
          }
        } else {
          yield chunk;
        }
      }

      if (abortController.signal.aborted && attempt < maxRetries - 1) {
        continue;
      }

      yield* buffer;
      return;
    } catch (error) {
      if (abortController.signal.aborted && attempt < maxRetries - 1) {
        continue;
      }
      throw error;
    }
  }
}
