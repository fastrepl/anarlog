import { useQuery } from "@tanstack/react-query";

import {
  persistShareRouteContinuation,
  restoreShareRouteContinuation,
} from "@/functions/share-route-continuation";
import {
  clearPersistedShareRouteToken,
  getShareRouteToken,
  retainShareRouteToken,
} from "@/lib/share-route-privacy";

export function useShareRouteContinuation(pathname: string) {
  const localToken = getShareRouteToken(pathname);
  const continuationQuery = useQuery({
    queryKey: ["share-route-continuation", pathname],
    queryFn: async () => {
      let token = localToken;
      if (!token) {
        token = await restoreShareRouteContinuation({
          data: pathname,
        });
        if (!token) {
          return null;
        }
        retainShareRouteToken(pathname, token);
      }

      const persisted = await persistShareRouteContinuation({
        data: { pathname, token },
      });
      if (!persisted) {
        throw new Error("share continuation unavailable");
      }

      clearPersistedShareRouteToken(pathname);
      return token;
    },
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
