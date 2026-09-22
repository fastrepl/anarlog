import { createBrowserClient, createServerClient } from "@supabase/ssr";
import { createClient } from "@supabase/supabase-js";
import { createClientOnlyFn, createServerOnlyFn } from "@tanstack/react-start";
import { getCookies, setCookie } from "@tanstack/react-start/server";

import { env, requireEnv } from "@/env";
import {
  filterInvalidSupabaseCookies,
  toSetCookieOptions,
} from "@/lib/supabase-cookies";

function getSupabaseCookies() {
  return filterInvalidSupabaseCookies(
    Object.entries(getCookies()).map(([name, value]) => ({ name, value })),
  );
}

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
          return getSupabaseCookies();
        },
        setAll(cookies) {
          cookies.forEach((cookie) => {
            setCookie(cookie.name, cookie.value, toSetCookieOptions(cookie));
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
          return getSupabaseCookies();
        },
        setAll(_cookies: Array<{ name: string; value: string }>) {},
      },
    },
  );
});

export const getSupabasePublicServerClient = createServerOnlyFn(() => {
  return createClient(
    requireEnv(env.SUPABASE_URL, "SUPABASE_URL"),
    requireEnv(env.SUPABASE_ANON_KEY, "SUPABASE_ANON_KEY"),
    {
      auth: {
        autoRefreshToken: false,
        persistSession: false,
      },
    },
  );
});

export const getSupabaseAdminClient = createServerOnlyFn(() => {
  return createClient(
    requireEnv(env.SUPABASE_URL, "SUPABASE_URL"),
    requireEnv(env.SUPABASE_SERVICE_ROLE_KEY, "SUPABASE_SERVICE_ROLE_KEY"),
    {
      auth: {
        autoRefreshToken: false,
        persistSession: false,
      },
    },
  );
});
