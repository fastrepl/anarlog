import { useQuery } from "@tanstack/react-query";

import { listMyWorkspaceInvitations, requireTeamContext } from "./client";

import { useAuth } from "~/auth";

export const MY_INVITATIONS_QUERY_KEY = "team-my-invitations";

export function useMyWorkspaceInvitations() {
  const auth = useAuth();
  const signedIn = Boolean(auth.supabase && auth.session);

  return useQuery({
    queryKey: [MY_INVITATIONS_QUERY_KEY, auth.session?.user.id],
    enabled: signedIn && !auth.session?.user.is_anonymous,
    queryFn: () => listMyWorkspaceInvitations(requireTeamContext(auth)),
    refetchInterval: 60_000,
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: true,
  });
}
