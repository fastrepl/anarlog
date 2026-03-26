import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useEffect, useRef } from "react";

import type { AuthCallbackSearch } from "@hypr/plugin-deeplink2";

import { useAuth } from "~/auth";

export const Route = createFileRoute("/auth/callback")({
  validateSearch: (search): AuthCallbackSearch => {
    return {
      access_token: (search as AuthCallbackSearch).access_token ?? "",
      refresh_token: (search as AuthCallbackSearch).refresh_token ?? "",
    };
  },
  component: AuthCallbackRoute,
});

function AuthCallbackRoute() {
  const search = Route.useSearch();
  const navigate = useNavigate();
  const auth = useAuth();
  const handledRef = useRef(false);

  useEffect(() => {
    if (handledRef.current) return;
    handledRef.current = true;

    const { access_token, refresh_token } = search;

    if (access_token && refresh_token && auth) {
      void auth
        .setSessionFromTokens(access_token, refresh_token)
        .finally(() => {
          void navigate({ to: "/app/main" });
        });
    } else {
      void navigate({ to: "/app/main" });
    }
  }, [search, auth, navigate]);

  return null;
}
