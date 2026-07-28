import type { ErrorRouteComponent } from "@tanstack/react-router";

import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";

const errorKeys = new WeakMap<object, number>();
let nextErrorKey = 0;

function getErrorKey(error: unknown): number | string {
  if (
    (typeof error === "object" && error !== null) ||
    typeof error === "function"
  ) {
    const object = error as object;
    let key = errorKeys.get(object);
    if (key === undefined) {
      key = nextErrorKey;
      nextErrorKey += 1;
      errorKeys.set(object, key);
    }
    return key;
  }

  return `${typeof error}:${String(error)}`;
}

function ErrorReporter({ error }: { error: unknown }) {
  useMountEffect(() => {
    captureOperationalError(error, {
      operation: "route_render",
    });
  });

  return null;
}

export const ErrorPage: ErrorRouteComponent = ({ error }) => {
  return (
    <main className="flex min-h-screen items-center justify-center bg-[#f7f2e8] px-5 text-center text-[#181613]">
      <ErrorReporter key={getErrorKey(error)} error={error} />
      <div>
        <p className="text-sm font-medium tracking-[0.18em] text-[#756b5d] uppercase">
          Something went wrong
        </p>
        <h1 className="mt-3 text-4xl font-semibold tracking-normal">
          We could not load this page.
        </h1>
        <button
          type="button"
          className="mt-6 inline-flex rounded-full bg-[#181613] px-5 py-3 text-sm font-medium text-white"
          onClick={() => window.location.reload()}
        >
          Try again
        </button>
      </div>
    </main>
  );
};
