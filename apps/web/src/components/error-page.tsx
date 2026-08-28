import * as stylex from "@stylexjs/stylex";
import type { ErrorRouteComponent } from "@tanstack/react-router";

import { radii } from "@anlg/design-system/tokens.stylex";

import { useMountEffect } from "@/hooks/useMountEffect";
import { captureOperationalError } from "@/lib/error-reporting";
const styles = stylex.create({
  style1: {
    display: "flex",
    minHeight: "100vh",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "#f7f2e8",
    paddingInline: "1.25rem",
    textAlign: "center",
    color: "#181613",
  },
  style2: {
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    letterSpacing: ".18em",
    color: "#756b5d",
    textTransform: "uppercase",
  },
  style3: {
    marginTop: ".75rem",
    fontSize: "2.25rem",
    lineHeight: "2.5rem",
    fontWeight: 600,
    letterSpacing: 0,
  },
  style4: {
    marginTop: "1.5rem",
    display: "inline-flex",
    borderRadius: radii.full,
    backgroundColor: "#181613",
    paddingInline: "1.25rem",
    paddingBlock: ".75rem",
    fontSize: ".875rem",
    lineHeight: "1.25rem",
    fontWeight: 500,
    color: "#fff",
  },
});
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
    <main {...stylex.props(styles.style1)}>
      <ErrorReporter key={getErrorKey(error)} error={error} />
      <div>
        <p {...stylex.props(styles.style2)}>Something went wrong</p>
        <h1 {...stylex.props(styles.style3)}>We could not load this page.</h1>
        <button
          type="button"
          {...stylex.props(styles.style4)}
          onClick={() => window.location.reload()}
        >
          Try again
        </button>
      </div>
    </main>
  );
};
