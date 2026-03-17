import { createServerFn } from "@tanstack/react-start";

import { getSupabaseServerClient } from "@/functions/supabase";
import { ADMIN_EMAILS } from "@/lib/team";

export const isAdminEmail = (email: string): boolean => {
  return ADMIN_EMAILS.includes(email.toLowerCase());
};

export const fetchAdminUser = createServerFn({ method: "GET" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const { data, error: _error } = await supabase.auth.getUser();

    if (!data.user?.email) {
      return null;
    }

    const email = data.user.email;
    const isAdmin = isAdminEmail(email);

    return {
      email,
      isAdmin,
    };
  },
);

export const fetchGitHubCredentials = createServerFn({ method: "GET" }).handler(
  async () => {
    const supabase = getSupabaseServerClient();
    const { data: userData } = await supabase.auth.getUser();

    if (!userData.user?.id) {
      return { hasCredentials: false, isValid: false };
    }

    const { data: admin } = await supabase
      .from("admins")
      .select("github_token, github_username")
      .eq("id", userData.user.id)
      .single();

    const hasCredentials = !!(admin?.github_token && admin?.github_username);

    if (!hasCredentials) {
      return { hasCredentials: false, isValid: false };
    }

    try {
      const response = await fetch("https://api.github.com/user", {
        headers: {
          Authorization: `Bearer ${admin.github_token}`,
          Accept: "application/vnd.github.v3+json",
        },
      });

      return {
        hasCredentials: true,
        isValid: response.ok,
      };
    } catch {
      return {
        hasCredentials: true,
        isValid: false,
      };
    }
  },
);
