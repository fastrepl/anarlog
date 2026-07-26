import type { SupabaseClient } from "@supabase/supabase-js";
import { createClient } from "@supabase/supabase-js";
import { createMiddleware } from "hono/factory";

import { createJwksVerifier } from "./jwt";

export type SupabaseAuthBindings = {
  Variables: {
    supabaseUserId: string;
    supabaseClient: SupabaseClient;
    entitlements: string[];
  };
};

export type SupabaseAuthConfig = {
  supabaseUrl: string;
  supabaseAnonKey: string;
};

export function createSupabaseAuthMiddleware<T extends SupabaseAuthBindings>(
  config: SupabaseAuthConfig,
) {
  const verifier = createJwksVerifier(config.supabaseUrl);

  return createMiddleware<T>(async (c, next) => {
    const authHeader = c.req.header("Authorization");
    if (!authHeader) {
      return c.text("unauthorized", 401);
    }

    const token = authHeader.replace(/^bearer /i, "");

    try {
      const payload = await verifier.verify(token);

      const userId = payload.sub;
      if (!userId) {
        return c.text("unauthorized", 401);
      }

      c.set("supabaseUserId", userId);
      c.set("entitlements", payload.entitlements ?? []);

      const supabaseClient = createClient(
        config.supabaseUrl,
        config.supabaseAnonKey,
        { global: { headers: { Authorization: authHeader } } },
      );
      c.set("supabaseClient", supabaseClient);

      await next();
    } catch {
      return c.text("unauthorized", 401);
    }
  });
}
