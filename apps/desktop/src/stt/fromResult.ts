import { Effect } from "effect";

import { type Result } from "@hypr/plugin-listener";

export const fromResult = <A, E>(
  promise: Promise<Result<A, E>>,
): Effect.Effect<A, E> =>
  Effect.tryPromise({
    try: () => promise,
    catch: (error) => error as E,
  }).pipe(
    Effect.flatMap((result) =>
      result.status === "ok"
        ? Effect.succeed(result.data)
        : Effect.fail(result.error),
    ),
  );
