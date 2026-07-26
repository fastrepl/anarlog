import { getSupabaseBrowserClient } from "@/functions/supabase";

export async function getAccessToken(): Promise<string> {
  const supabase = getSupabaseBrowserClient();
  const { data } = await supabase.auth.getSession();
  const token = data?.session?.access_token;
  if (!token) {
    throw new Error("Not authenticated");
  }
  return token;
}
