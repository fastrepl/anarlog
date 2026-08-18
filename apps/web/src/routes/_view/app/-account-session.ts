import { useQuery } from "@tanstack/react-query";
import { jwtDecode } from "jwt-decode";

import { deriveBillingInfo, type SupabaseJwtPayload } from "@anlg/supabase";

import { getSupabaseBrowserClient } from "@/functions/supabase";

export const accountSessionQueryKey = ["account-session"];

export function useAccountSession() {
  return useQuery({
    queryKey: accountSessionQueryKey,
    // Skip the SSR fetch: the browser-only Supabase client throws on the
    // server, and this data is session-scoped anyway.
    enabled: typeof window !== "undefined",
    queryFn: async () => {
      const supabase = getSupabaseBrowserClient();
      const { data } = await supabase.auth.getSession();
      const session = data.session;
      if (!session) {
        return null;
      }

      const metadata: Record<string, unknown> =
        session.user.user_metadata ?? {};
      const metadataString = (key: string) => {
        const value = metadata[key];
        return typeof value === "string" && value.trim() ? value.trim() : null;
      };

      return {
        billing: deriveBillingInfo(
          jwtDecode<SupabaseJwtPayload>(session.access_token),
        ),
        createdAt: session.user.created_at ?? null,
        profile: {
          // An explicit full_name (even cleared to null) wins; fall back to
          // the OAuth-provided name only when the user never set one.
          fullName:
            "full_name" in metadata
              ? metadataString("full_name")
              : metadataString("name"),
          linkedinUrl: metadataString("linkedin_url"),
          xHandle: metadataString("x_handle"),
        },
      };
    },
  });
}
