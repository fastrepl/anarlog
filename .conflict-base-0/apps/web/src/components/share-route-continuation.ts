import { useQuery } from "@tanstack/react-query";

import {
  persistShareRouteContinuation,
  restoreShareRouteContinuation,
} from "@/functions/share-route-continuation";
import {
  clearPersistedShareRouteToken,
  getShareRouteToken,
  loadShareRouteContinuation,
  retainShareRouteToken,
} from "@/lib/share-route-privacy";

export function useShareRouteContinuation(pathname: string) {
  const localToken = getShareRouteToken(pathname);
  const continuationQuery = useQuery({
    queryKey: ["share-route-continuation", pathname],
    queryFn: ({ signal }) =>
      loadShareRouteContinuation({
        clearPersisted: () => clearPersistedShareRouteToken(pathname),
        localToken,
        persist: (token) =>
          persistShareRouteContinuation({
            data: { pathname, token },
          }),
        restore: () =>
          restoreShareRouteContinuation({
            data: pathname,
          }),
        retain: (token) => retainShareRouteToken(pathname, token),
        signal,
      }),
    gcTime: 0,
    retry: false,
    staleTime: Infinity,
  });

  return {
    isError: continuationQuery.isError,
    isPending: continuationQuery.isPending,
    retry: continuationQuery.refetch,
    token: continuationQuery.data ?? null,
  };
}
