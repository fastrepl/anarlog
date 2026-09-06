import { useQuery } from "@tanstack/react-query";
import { fetch } from "expo/fetch";

import { supabase } from "@/auth/client";
import { useAuth } from "@/auth/context";
import { enrollInTrial } from "@/auth/trial";
import { env } from "@/lib/env";

export function useTrial() {
  const auth = useAuth();
  const accountId = auth.session?.user.id;
  return useQuery({
    queryKey: ["mobile-pro-trial", accountId],
    enabled: Boolean(accountId) && !auth.bypass && !auth.billing.isPaid,
    staleTime: Infinity,
    retry: 1,
    queryFn: async ({ signal }) => {
      const current = await supabase!.auth.getSession();
      const session = current.data.session;
      if (current.error || !session || session.user.id !== accountId)
        throw new Error("Sign in again to start your Pro trial.");
      const timeout = new AbortController();
      const abort = () => timeout.abort();
      signal.addEventListener("abort", abort);
      const timer = setTimeout(abort, 20_000);
      try {
        if (signal.aborted) timeout.abort();
        const started = await enrollInTrial({
          apiUrl: env.apiUrl,
          accessToken: session.access_token,
          signal: timeout.signal,
          request: fetch,
        });
        const latest = await supabase!.auth.getSession();
        if (signal.aborted || latest.data.session?.user.id !== accountId)
          return started;
        // A previous attempt or desktop may have already started this trial.
        const unlocked = await auth.refreshBilling();
        if (started && !unlocked && !signal.aborted)
          throw new Error(
            "Your trial has started. Refresh your plan to finish activating sync and Anarlog models.",
          );
        return started;
      } finally {
        clearTimeout(timer);
        signal.removeEventListener("abort", abort);
      }
    },
  });
}
