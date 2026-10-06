import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { isTauri } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { z } from "zod";

import { referralSupportUrl } from "@anlg/supabase/referrals";
import { ArrowsClockwise, Check, Copy } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { toast } from "@anlg/ui/components/ui/toast";

import { useAuth } from "~/auth/auth-context";
import { env } from "~/env";
import { SettingsPageTitle } from "~/settings/page-title";
import { useTabs } from "~/store/zustand/tabs";

const summarySchema = z.object({
  enabled: z.boolean(),
  eligible: z.boolean(),
  code: z
    .string()
    .regex(/^[a-f0-9]{24}$/)
    .nullable(),
  accepted: z.number().int().min(0).max(3),
  remaining: z.number().int().min(0).max(3),
  months_earned: z.number().int().min(0).max(3),
  invites: z.array(
    z.object({
      id: z.string(),
      slot: z.number(),
      policy: z.enum(["legacy_payment", "trial_month"]),
      accepted_at: z.string(),
      extended_until: z.string().nullable(),
      legacy_amount_cents: z.number(),
      status: z.enum([
        "accepted",
        "applying",
        "applied",
        "pending",
        "legacy_pending",
        "legacy_applied",
      ]),
    }),
  ),
});

export function SettingsReferrals() {
  const { session } = useAuth();
  return (
    <div className="@container flex w-full min-w-0 flex-col gap-8">
      <SettingsPageTitle title={<Trans>Invite friends</Trans>} />
      <ReferralInvites key={session?.user.id ?? "signed-out"} />
      <div className="text-muted-foreground flex flex-col gap-2 text-sm">
        <h3 className="text-foreground font-medium">
          <Trans>Missing a referral reward?</Trans>
        </h3>
        <p>
          <Trans>
            If your friend signed up through your link and started their free
            trial, but your free month hasn't appeared, email us and we'll help.
          </Trans>
        </p>
        <a className="w-fit underline" href={referralSupportUrl}>
          <Trans>Email us</Trans>
        </a>
      </div>
    </div>
  );
}

function ReferralInvites() {
  const { t } = useLingui();
  const auth = useAuth();
  const openNew = useTabs((s) => s.openNew);
  const userId = auth.session?.user.id;
  const signedIn = !!auth.session && !auth.session.user.is_anonymous;
  const signIn = useMutation({
    mutationFn: () => auth.signIn(),
    onError: () => toast.error(t`Couldn't open sign-in. Try again.`),
  });
  // Credentials are request context, while cached data belongs to the account.
  // eslint-disable-next-line @tanstack/query/exhaustive-deps
  const summary = useQuery({
    queryKey: ["referral-summary", userId],
    enabled: signedIn && !!auth.supabase,
    queryFn: async ({ signal }) => {
      const { supabase, session } = auth;
      if (!supabase || !session || session.user.is_anonymous)
        throw new Error("Unauthorized");
      const { data, error } = await supabase
        .rpc("get_referral_summary")
        .setHeader("Authorization", `Bearer ${session.access_token}`)
        .abortSignal(signal);
      if (error) throw error;
      return summarySchema.parse(data);
    },
    staleTime: 15_000,
    gcTime: 0,
    retry: 1,
    refetchOnWindowFocus: true,
    refetchInterval: (query) =>
      query.state.data?.invites.some((i) =>
        ["accepted", "applying"].includes(i.status),
      )
        ? 15_000
        : false,
  });
  const copy = useMutation({
    mutationFn: async () => {
      if (
        !summary.data?.eligible ||
        !summary.data.code ||
        summary.data.remaining === 0
      )
        throw new Error("No invites available");
      const url = new URL(
        `/invite/${summary.data.code}`,
        env.VITE_APP_URL,
      ).toString();
      if (isTauri()) await writeText(url);
      else await navigator.clipboard.writeText(url);
    },
    onSuccess: () => toast.success(t`Invite link copied`),
    onError: () => toast.error(t`Couldn't copy the invite link. Try again.`),
  });
  if (auth.session === undefined)
    return (
      <p role="status">
        <Trans>Loading your account...</Trans>
      </p>
    );
  if (!signedIn)
    return (
      <div className="flex flex-col items-start gap-3">
        <p>
          <Trans>Sign in to see your referral invites.</Trans>
        </p>
        <Button
          variant="outline"
          disabled={signIn.isPending}
          onClick={() => signIn.mutate()}
        >
          <Trans>Sign in</Trans>
        </Button>
        {signIn.isSuccess && (
          <p role="status">
            <Trans>Finish in your browser, then return to Anarlog.</Trans>
          </p>
        )}
      </div>
    );
  if (!auth.supabase || summary.isError)
    return (
      <div role="alert" className="flex flex-col items-start gap-3">
        <p>
          <Trans>We couldn't load your invites. Please try again.</Trans>
        </p>
        <Button variant="outline" onClick={() => void summary.refetch()}>
          <Trans>Try again</Trans>
        </Button>
      </div>
    );
  if (summary.isPending)
    return (
      <p role="status">
        <Trans>Loading your invites...</Trans>
      </p>
    );
  const data = summary.data;
  const accepted = data.accepted;
  const remaining = data.remaining;
  const earned = data.months_earned;
  return (
    <section className="flex flex-col gap-6">
      {data.enabled ? (
        <div className="border-border bg-muted/30 flex flex-col gap-3 rounded-xl border p-6">
          <h3 className="text-xl font-medium tracking-tight">
            <Trans>Refer friends, get a month free</Trans>
          </h3>
          <p className="text-muted-foreground text-sm leading-relaxed">
            <Trans>
              Get one free month when a friend signs up through your link and
              starts their free trial. Invite up to 3 friends. Each friend gets
              30 days of Pro free.
            </Trans>
          </p>
          <p className="text-muted-foreground text-xs">
            <Trans>
              Your trial or paid subscription is extended by one month. No
              payment from your friend is required.
            </Trans>
          </p>
        </div>
      ) : (
        <p role="status">
          <Trans>
            The new referral offer is not available yet. Your previous referrals
            are shown below.
          </Trans>
        </p>
      )}
      {data.enabled && (
        <div className="grid grid-cols-1 gap-3 @sm:grid-cols-3">
          <SummaryStat
            label={<Trans>Accepted</Trans>}
            value={`${accepted} / 3`}
          />
          <SummaryStat
            label={<Trans>Free months earned</Trans>}
            value={earned}
          />
          <SummaryStat
            label={<Trans>Invites remaining</Trans>}
            value={remaining}
          />
        </div>
      )}
      {data.enabled &&
        (data.remaining === 0 ? (
          <p>
            <Trans>All 3 invites have been accepted.</Trans>
          </p>
        ) : data.eligible && data.code ? (
          <div className="border-border flex flex-wrap items-center gap-3 rounded-xl border p-4">
            <span className="min-w-0 flex-1 text-sm break-all">
              {new URL(`/invite/${data.code}`, env.VITE_APP_URL).toString()}
            </span>
            <Button
              variant="outline"
              disabled={copy.isPending}
              onClick={() => copy.mutate()}
            >
              {copy.isSuccess ? (
                <Check className="size-4" />
              ) : (
                <Copy className="size-4" />
              )}
              {copy.isSuccess ? (
                <Trans>Copied</Trans>
              ) : (
                <Trans>Copy link</Trans>
              )}
            </Button>
          </div>
        ) : (
          <div className="flex flex-col items-start gap-3">
            <p className="text-muted-foreground text-sm">
              <Trans>
                New referral invites are available with an active personal Pro
                trial or paid subscription. Team-only subscriptions aren't
                eligible.
              </Trans>
            </p>
            <Button
              variant="outline"
              onClick={() =>
                openNew({ type: "settings", state: { tab: "billing" } })
              }
            >
              <Trans>View billing</Trans>
            </Button>
          </div>
        ))}
      <div className="flex items-center justify-between">
        <h3 className="font-medium">
          <Trans>Your referrals</Trans>
        </h3>
        <Button
          variant="ghost"
          size="sm"
          disabled={summary.isFetching}
          onClick={() => void summary.refetch()}
        >
          <ArrowsClockwise className="size-4" />
          <Trans>Refresh</Trans>
        </Button>
      </div>
      {data.invites.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          <Trans>
            No accepted invites yet. Copying your link doesn't count as an
            accepted invite.
          </Trans>
        </p>
      ) : (
        <ul className="border-border divide-border divide-y rounded-xl border">
          {data.invites.map((invite) => (
            <ReferralRow key={invite.id} invite={invite} />
          ))}
        </ul>
      )}
      {data.invites.some((i) => i.policy === "legacy_payment") && (
        <p className="text-muted-foreground text-xs">
          <Trans>
            Previous referrals keep their original reward terms and count toward
            your 3-invite limit.
          </Trans>
        </p>
      )}
    </section>
  );
}

function SummaryStat({
  label,
  value,
}: {
  label: React.ReactNode;
  value: React.ReactNode;
}) {
  return (
    <div className="border-border rounded-xl border p-4">
      <p className="text-muted-foreground text-xs">{label}</p>
      <p className="mt-2 text-xl font-medium">{value}</p>
    </div>
  );
}

function ReferralRow({
  invite,
}: {
  invite: z.infer<typeof summarySchema>["invites"][number];
}) {
  const { t, i18n } = useLingui();
  const slot = invite.slot;
  const labels = {
    accepted: t`Accepted · Waiting for trial to start`,
    applying: t`Trial started · Applying your free month`,
    applied: t`Reward applied · 1 free month added`,
    pending: t`Reward pending · Contact us for help`,
    legacy_pending: t`Previous referral offer · Waiting for first payment`,
    legacy_applied: t`Previous referral offer · Reward applied`,
  };
  const amount = new Intl.NumberFormat(i18n.locale, {
    style: "currency",
    currency: "USD",
  }).format(invite.legacy_amount_cents / 100);
  const end = invite.extended_until
    ? new Date(invite.extended_until).toLocaleDateString(i18n.locale)
    : null;
  return (
    <li className="flex flex-col gap-2 p-4">
      <p className="text-sm font-medium">
        <Trans>Invite {slot}</Trans>
        <span className="text-muted-foreground ml-3 font-normal">
          {new Date(invite.accepted_at).toLocaleDateString(i18n.locale)}
        </span>
      </p>
      <p className="text-muted-foreground text-sm">{labels[invite.status]}</p>
      {invite.policy === "legacy_payment" && (
        <p className="text-muted-foreground text-xs">
          <Trans>
            Original offer: {amount} off your subscription after your friend's
            first payment.
          </Trans>
        </p>
      )}
      {end && (
        <p className="text-muted-foreground text-xs">
          <Trans>Extended through {end} when this reward was applied.</Trans>
        </p>
      )}
    </li>
  );
}
