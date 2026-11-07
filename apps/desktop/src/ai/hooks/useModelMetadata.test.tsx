import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { expect, it } from "vitest";

import { useModelMetadata } from "./useModelMetadata";

it("loads the new catalog when the endpoint or credential scope changes", async () => {
  const client = new QueryClient();
  const original = async () => ({
    models: ["original-model"],
    ignored: [],
    metadata: {},
  });
  const replacement = async () => ({
    models: ["replacement-model"],
    ignored: [],
    metadata: {},
  });
  const { result, rerender, unmount } = renderHook(
    ({ scope, listModels }) =>
      useModelMetadata("custom", listModels, { scope }),
    {
      initialProps: {
        scope: "original-endpoint-and-credential",
        listModels: original,
      },
      wrapper: ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      ),
    },
  );
  await waitFor(() =>
    expect(result.current.data?.models).toEqual(["original-model"]),
  );
  rerender({
    scope: "replacement-endpoint-and-credential",
    listModels: replacement,
  });
  await waitFor(() =>
    expect(result.current.data?.models).toEqual(["replacement-model"]),
  );
  unmount();
  client.clear();
});
