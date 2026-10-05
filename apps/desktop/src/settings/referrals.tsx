import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { z } from "zod";

import { ArrowsClockwise, Check, Copy } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { toast } from "@anlg/ui/components/ui/toast";

import { useAuth } from "~/auth/auth-context";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";
import { useTabs } from "~/store/zustand/tabs";

const referralInviteSchema = z.object({
  slot: z.number().int().min(1).max(3),
  code: z.string().regex(/^[a-f0-9]{24}$/),
  status: z.enum(["available", "trial_started", "reward_earned"]),
});

export function SettingsReferrals() {
  const { session } = useAuth();

  return (
    <div className="@container flex w-full min-w-0 flex-col gap-8">
      <SettingsPageTitle title={<Trans>Invite friends</Trans>} />

      <section className="border-border bg-muted/30 flex flex-col gap-5 rounded-xl border p-6">
        <div className="flex flex-col gap-2">
          <h3 className="text-xl font-medium tracking-tight">
            <Trans>Share your referral link</Trans>
          </h3>
          <p className="text-muted-foreground text-sm leading-relaxed">
            <Trans>
              Give a friend 30 days of Anarlog Pro. Get $14 off your
              subscription after their first payment.
            </Trans>
          </p>
        </div>
        <div className="border-border grid gap-4 border-t pt-5 @sm:grid-cols-2">
          <div className="flex flex-col gap-1">
            <p className="text-muted-foreground text-xs">
              <Trans>For your friend</Trans>
            </p>
            <p className="text-sm font-medium">
              <Trans>30 days of Pro, free</Trans>
            </p>
          </div>
          <div className="flex flex-col gap-1">
            <p className="text-muted-foreground text-xs">
              <Trans>For you</Trans>
            </p>
            <p className="text-sm font-medium">
              <Trans>$14 off your subscription</Trans>
            </p>
          </div>
        </div>
      </section>

      <ReferralInvites key={session?.user.id ?? "signed-out"} />
    </div>
  );
}

function ReferralInvites() {
  const { t } = useLingui();
  const auth = useAuth();
  const openNew = useTabs((state) => state.openNew);
  const userId = auth.session?.user.id;
  const signedIn = !!auth.session && !auth.session.user.is_anonymous;
  const signIn = useMutation({
    mutationFn: () => auth.signIn(),
    onError: () => toast.error(t`Couldn't open sign-in. Try again.`),
  });
  // Credentials are request context; invite data belongs to the user, not the token.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  const invites = useQuery({
    queryKey: ["referral-invites", userId],
    enabled: signedIn && !!auth.supabase,
    queryFn: async ({ signal }) => {
      const { supabase, session } = auth;
      if (!supabase || !session || session.user.is_anonymous) {
        throw new Error("Unauthorized");
      }
      const { data, error } = await supabase
        .rpc("get_or_create_referral_invites")
        .setHeader("Authorization", `Bearer ${session.access_token}`)
        .abortSignal(signal);
      if (error) throw error;
      return z.array(referralInviteSchema).parse(data);
    },
    staleTime: 30_000,
    gcTime: 0,
    retry: 1,
    refetchOnWindowFocus: true,
  });

  return (
    <section
      className="flex flex-col gap-4"
      aria-labelledby="referral-invites-title"
    >
      <div className="flex items-center justify-between gap-2">
        <h3 id="referral-invites-title" className="text-sm font-medium">
          <Trans>Your invites</Trans>
        </h3>
        {signedIn && auth.supabase && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            disabled={invites.isFetching}
            onClick={() => void invites.refetch()}
          >
            <ArrowsClockwise className="size-4" aria-hidden="true" />
            <Trans>Refresh</Trans>
          </Button>
        )}
      </div>
      {auth.session === undefined ? (
        <p role="status" className="text-muted-foreground text-sm">
          <Trans>Loading your account...</Trans>
        </p>
      ) : !signedIn ? (
        <div className="flex flex-col items-start gap-3">
          <p className="text-muted-foreground text-sm">
            <Trans>Sign in to see your referral invites.</Trans>
          </p>
          <Button
            type="button"
            variant="outline"
            disabled={signIn.isPending}
            onClick={() => signIn.mutate()}
          >
            <Trans>Sign in</Trans>
          </Button>
          {signIn.isSuccess && (
            <p role="status" className="text-muted-foreground text-sm">
              <Trans>Finish in your browser, then return to Anarlog.</Trans>
            </p>
          )}
        </div>
      ) : !auth.supabase ? (
        <p role="alert" className="text-muted-foreground text-sm">
          <Trans>
            Referral invites are unavailable. Check your connection and try
            again.
          </Trans>
        </p>
      ) : invites.isPending ? (
        <p role="status" className="text-muted-foreground text-sm">
          <Trans>Loading your invites...</Trans>
        </p>
      ) : invites.isError ? (
        <div role="alert" className="flex flex-col items-start gap-3">
          <p className="text-muted-foreground text-sm">
            <Trans>We couldn't load your invites. Please try again.</Trans>
          </p>
          <Button
            type="button"
            variant="outline"
            disabled={invites.isFetching}
            onClick={() => void invites.refetch()}
          >
            <Trans>Try again</Trans>
          </Button>
        </div>
      ) : invites.data.length === 0 ? (
        <div className="flex flex-col items-start gap-3">
          <p className="text-muted-foreground text-sm">
            <Trans>
              No invites are available for this account. Referral invites
              require an active paid subscription.
            </Trans>
          </p>
          <Button
            type="button"
            variant="outline"
            onClick={() =>
              openNew({ type: "settings", state: { tab: "billing" } })
            }
          >
            <Trans>View billing</Trans>
          </Button>
        </div>
      ) : (
        <ul className="border-border divide-border divide-y rounded-xl border">
          {invites.data.map((invite) => (
            <ReferralInvite key={invite.code} invite={invite} />
          ))}
        </ul>
      )}
      <p className="text-muted-foreground text-xs leading-relaxed">
        <Trans>
          Each invite is for one friend who is new to Anarlog. Referral invites
          are available to Pro subscribers.
        </Trans>
      </p>
    </section>
  );
}

function ReferralInvite({
  invite,
}: {
  invite: z.infer<typeof referralInviteSchema>;
}) {
  const { t } = useLingui();
  const copy = useMutation({
    mutationFn: async () => {
      const url = new URL(
        `/invite/${invite.code}`,
        env.VITE_APP_URL,
      ).toString();
      if (isTauri()) {
        await writeText(url);
      } else {
        await navigator.clipboard.writeText(url);
      }
    },
    onSuccess: () => toast.success(t`Invite link copied`),
    onError: () => toast.error(t`Couldn't copy the invite link. Try again.`),
  });
  const slot = invite.slot;
  const status =
    invite.status === "reward_earned"
      ? t`Reward applied`
      : invite.status === "trial_started"
        ? t`Invite accepted`
        : t`Available`;

  return (
    <li className="flex flex-wrap items-center justify-between gap-3 p-4">
      <div className="flex items-center gap-3">
        <span
          className="bg-muted text-muted-foreground flex size-8 shrink-0 items-center justify-center rounded-full text-xs"
          aria-hidden="true"
        >
          {slot}
        </span>
        <div className="flex flex-col gap-1">
          <p className="text-sm font-medium">
            <Trans>Invite {slot}</Trans>
          </p>
          <p className="text-muted-foreground text-xs">{status}</p>
        </div>
      </div>
      {invite.status === "available" ? (
        <Button
          type="button"
          variant="outline"
          disabled={copy.isPending}
          onClick={() => copy.mutate()}
        >
          {copy.isSuccess ? (
            <Check className="size-4" aria-hidden="true" />
          ) : (
            <Copy className="size-4" aria-hidden="true" />
          )}
          {copy.isSuccess ? <Trans>Copied</Trans> : <Trans>Copy link</Trans>}
        </Button>
      ) : (
        <Check className="text-muted-foreground size-4" aria-hidden="true" />
      )}
    </li>
  );
}
