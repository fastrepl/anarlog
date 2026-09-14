import { Trans } from "@lingui/react/macro";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  getWorkspaceEmailAutoJoin,
  requireTeamContext,
  setWorkspaceEmailAutoJoin,
} from "./client";

import { useAuth } from "~/auth";
import { SettingSwitchRow } from "~/settings/setting-row";

export function WorkspaceEmailAutoJoin({
  workspaceId,
}: {
  workspaceId: string;
}) {
  const auth = useAuth();
  const queryClient = useQueryClient();
  const queryKey = ["team-email-auto-join", workspaceId, auth.session?.user.id];
  // Cache by identity, without putting Supabase credentials in query keys.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  const setting = useQuery({
    queryKey,
    queryFn: () =>
      getWorkspaceEmailAutoJoin(requireTeamContext(auth), workspaceId),
    retry: false,
  });
  const save = useMutation({
    mutationFn: (enabled: boolean) =>
      setWorkspaceEmailAutoJoin(requireTeamContext(auth), workspaceId, enabled),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });
  const domain = setting.data?.domain;

  return (
    <div className="flex flex-col gap-2">
      <SettingSwitchRow
        title={<Trans>Join automatically with a work email</Trans>}
        description={
          domain ? (
            <Trans>
              People with a verified @{domain} email join this team when they
              sign in. Billing starts when they join and is prorated for the
              remaining billing period.
            </Trans>
          ) : setting.isPending || setting.isError ? undefined : (
            <Trans>
              Use a verified work email to enable automatic joining. Personal
              email providers such as Gmail are excluded.
            </Trans>
          )
        }
        checked={setting.data?.enabled === true}
        onChange={(enabled) => save.mutate(enabled)}
        disabled={
          !setting.data || save.isPending || (!domain && !setting.data.enabled)
        }
      />
      {setting.isError || save.isError ? (
        <p role="alert" className="text-destructive text-sm">
          {(save.error ?? setting.error)?.message}
        </p>
      ) : null}
    </div>
  );
}
