import { queryOptions, useQuery } from "@tanstack/react-query";

import {
  DEFAULT_RESULT,
  type ListModelsResult,
} from "~/settings/ai/shared/list-common";

export function modelMetadataQueryOptions(
  providerId: string | null,
  listModels: (() => Promise<ListModelsResult> | ListModelsResult) | undefined,
  scope?: string,
) {
  // scope is the credential and endpoint identity; the catalog outlives closure recreation.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  return queryOptions({
    queryKey: ["models", providerId, scope],
    queryFn: async () => (listModels ? await listModels() : DEFAULT_RESULT),
    retry: 3,
    retryDelay: 300,
    staleTime: 30_000,
  });
}

export function useModelMetadata(
  providerId: string | null,
  listModels: (() => Promise<ListModelsResult> | ListModelsResult) | undefined,
  options?: {
    enabled?: boolean;
    scope?: string;
  },
) {
  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    ...modelMetadataQueryOptions(providerId, listModels, options?.scope),
    enabled: options?.enabled ?? Boolean(providerId && listModels),
  });

  return { data, isLoading, isError, refetch, isFetching };
}
