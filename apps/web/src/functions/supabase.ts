import { createBrowserClient, createServerClient } from "@supabase/ssr";
import { createClient } from "@supabase/supabase-js";
import { createClientOnlyFn, createServerOnlyFn } from "@tanstack/react-start";
import { getCookies, setCookie } from "@tanstack/react-start/server";

import { env, requireEnv } from "@/env";

export const getSupabaseBrowserClient = createClientOnlyFn(() => {
  return createBrowserClient(
    requireEnv(env.VITE_SUPABASE_URL, "VITE_SUPABASE_URL"),
    requireEnv(env.VITE_SUPABASE_ANON_KEY, "VITE_SUPABASE_ANON_KEY"),
    {
      auth: {
        detectSessionInUrl: true,
        flowType: "pkce",
      },
    },
  );
});

export const getSupabaseServerClient = createServerOnlyFn(() => {
  return createServerClient(
    requireEnv(env.SUPABASE_URL, "SUPABASE_URL"),
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY"),
    {
      auth: {
        autoRefreshToken: false,
      },
      cookies: {
        getAll() {
          return Object.entries(getCookies()).map(([name, value]) => ({
            name,
            value,
          }));
        },
        setAll(cookies: Array<{ name: string; value: string }>) {
          cookies.forEach((cookie) => {
            setCookie(cookie.name, cookie.value);
          });
        },
      },
    },
  );
});

export const getSupabaseDesktopFlowClient = createServerOnlyFn(() => {
  return createServerClient(
    requireEnv(env.SUPABASE_URL, "SUPABASE_URL"),
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY"),
    {
      auth: {
        autoRefreshToken: false,
      },
      cookies: {
        getAll() {
          return Object.entries(getCookies()).map(([name, value]) => ({
            name,
            value,
          }));
        },
        setAll(_cookies: Array<{ name: string; value: string }>) {},
      },
    },
  );
});

export const getSupabaseAdminClient = createServerOnlyFn(() => {
  const key =
    env.SUPABASE_SERVICE_ROLE_KEY ||
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY");
  return createClient(requireEnv(env.SUPABASE_URL, "SUPABASE_URL"), key, {
    auth: {
      autoRefreshToken: false,
      persistSession: false,
    },
  });
});
