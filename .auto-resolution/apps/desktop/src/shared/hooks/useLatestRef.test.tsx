import { render, renderHook } from "@testing-library/react";
import { Suspense } from "react";
import { describe, expect, it } from "vitest";

import { useLatestRef } from "./useLatestRef";

describe("useLatestRef", () => {
  it("keeps a stable ref synchronized after commit", () => {
    const { result, rerender } = renderHook(
      ({ value }: { value: string }) => useLatestRef(value),
      { initialProps: { value: "first" } },
    );
    const ref = result.current;

    rerender({ value: "second" });

    expect(result.current).toBe(ref);
    expect(ref.current).toBe("second");
  });

  it("does not expose values from a suspended render", () => {
    const refs: Array<{ current: string }> = [];
    const pending = new Promise<never>(() => {});

    function Harness({ value, suspend }: { value: string; suspend: boolean }) {
      refs.push(useLatestRef(value));
      if (suspend) {
        throw pending;
      }
      return null;
    }

    const { rerender } = render(
      <Suspense fallback={null}>
        <Harness value="first" suspend={false} />
      </Suspense>,
    );
    const committedRef = refs[0];

    rerender(
      <Suspense fallback={null}>
        <Harness value="second" suspend />
      </Suspense>,
    );

    expect(committedRef.current).toBe("first");
  });
});
