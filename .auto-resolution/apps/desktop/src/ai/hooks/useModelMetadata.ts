import { useQuery } from "@tanstack/react-query";

import {
  DEFAULT_RESULT,
  type ListModelsResult,
} from "~/settings/ai/shared/list-common";

export function useModelMetadata(
  providerId: string | null,
  listModels: (() => Promise<ListModelsResult> | ListModelsResult) | undefined,
  options?: {
    enabled?: boolean;
  },
) {
  const enabled = options?.enabled ?? Boolean(providerId && listModels);

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ["models", providerId, listModels],
    queryFn: async () => {
      if (!listModels) {
        return DEFAULT_RESULT;
      }
      return await listModels();
    },
    enabled,
    retry: 3,
    retryDelay: 300,
    staleTime: 1000 * 2,
  });

  return { data, isLoading, refetch, isFetching };
}
