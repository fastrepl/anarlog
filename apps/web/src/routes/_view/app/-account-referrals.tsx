import { useMutation, useQuery } from "@tanstack/react-query";
import { useRef } from "react";

import {
  referralSupportUrl,
  type ReferralSummary,
} from "@anlg/supabase/referrals";

import { getReferralInvites } from "@/functions/referrals";
import { useMountEffect } from "@/hooks/useMountEffect";
import { capturePrivateRouteEvent } from "@/lib/private-route-analytics";
import { observeReferralView } from "@/lib/referral-visibility";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export function ReferralSection({ ineligible }: { ineligible: boolean }) {
  const session = useAccountSession();
  const sectionRef = useRef<HTMLDivElement>(null);
  useMountEffect(() => {
    if (!sectionRef.current) return;
    return observeReferralView(sectionRef.current, () =>
      capturePrivateRouteEvent("referral_page_viewed"),
    );
  });
  const summary = useQuery({
    queryKey: ["referral-summary", session.data?.userId],
    enabled: typeof window !== "undefined" && !!session.data,
    queryFn: () => getReferralInvites(),
    gcTime: 0,
    refetchOnWindowFocus: true,
    refetchInterval: (query) =>
      query.state.data?.invites.some((i) =>
        ["accepted", "applying"].includes(i.status),
      )
        ? 15_000
        : false,
  });
  const copy = useMutation({
    onSuccess: () => capturePrivateRouteEvent("referral_link_copied"),
    mutationFn: async () => {
      if (
        !summary.data?.code ||
        !summary.data.eligible ||
        summary.data.remaining === 0
      )
        throw new Error("No invites available");
      await navigator.clipboard.writeText(
        new URL(
          `/invite/${summary.data.code}`,
          window.location.origin,
        ).toString(),
      );
    },
  });
  const data = summary.data;
  return (
    <div ref={sectionRef} className={accountCardClassName}>
      <div className="flex flex-col gap-5 p-6 sm:p-8">
        {ineligible && (
          <p className="text-color-muted text-sm">
            Referral invites are for new accounts. You can view your own
            referrals below.
          </p>
        )}
        {summary.isPending ? (
          <p role="status">Loading your referrals...</p>
        ) : summary.isError || !data ? (
          <div role="alert">
            <p>We couldn't load your referrals.</p>
            <button
              type="button"
              onClick={() => void summary.refetch()}
              className={accountPillSecondaryClassName}
            >
              Try again
            </button>
          </div>
        ) : (
          <>
            {data.enabled ? (
              <>
                <h3 className="text-lg font-medium">
                  Refer friends, get a month free
                </h3>
                <p className="text-color-muted text-sm">
                  Get one free month when a friend signs up through your link
                  and starts their free trial. Invite up to 3 friends. Each
                  friend gets 30 days of Pro free.
                </p>
                <p className="text-color-muted text-sm">
                  Your trial or paid subscription is extended by one month. No
                  payment from your friend is required.
                </p>
                <dl className="grid grid-cols-3 gap-3 text-sm">
                  <div>
                    <dt>Accepted</dt>
                    <dd className="mt-2 text-xl">{data.accepted} / 3</dd>
                  </div>
                  <div>
                    <dt>Free months earned</dt>
                    <dd className="mt-2 text-xl">{data.months_earned}</dd>
                  </div>
                  <div>
                    <dt>Invites remaining</dt>
                    <dd className="mt-2 text-xl">{data.remaining}</dd>
                  </div>
                </dl>
                {data.remaining === 0 ? (
                  <p>All 3 invites have been accepted.</p>
                ) : data.eligible && data.code ? (
                  <div className="flex flex-wrap items-center gap-3">
                    <span className="min-w-0 flex-1 text-sm break-all">
                      {new URL(
                        `/invite/${data.code}`,
                        window.location.origin,
                      ).toString()}
                    </span>
                    <button
                      type="button"
                      disabled={copy.isPending}
                      onClick={() => copy.mutate()}
                      className={accountPillSecondaryClassName}
                    >
                      {copy.isSuccess ? "Copied" : "Copy link"}
                    </button>
                    {copy.isError && (
                      <p role="alert">
                        Couldn't copy the link. Please try again.
                      </p>
                    )}
                  </div>
                ) : (
                  <p className="text-color-muted text-sm">
                    New referral invites require an active personal Pro trial or
                    paid subscription. Team-only subscriptions aren't eligible.
                  </p>
                )}
              </>
            ) : (
              <p>
                The new referral offer is not available yet. Your previous
                referrals are shown below.
              </p>
            )}
            <div className="flex items-center justify-between">
              <h3>Your referrals</h3>
              <button
                type="button"
                disabled={summary.isFetching}
                onClick={() => void summary.refetch()}
                className={accountPillSecondaryClassName}
              >
                Refresh
              </button>
            </div>
            {data.invites.length ? (
              <ul className="divide-border-subtle divide-y">
                {data.invites.map((invite) => (
                  <li
                    key={invite.id}
                    className="flex flex-col gap-2 py-4 text-sm"
                  >
                    <p>
                      Invite {invite.slot} ·{" "}
                      {new Date(invite.accepted_at).toLocaleDateString()}
                    </p>
                    <p className="text-color-muted">
                      {statusLabel(invite.status)}
                    </p>
                    {invite.policy === "legacy_payment" && (
                      <p className="text-color-muted">
                        Original offer: $
                        {(invite.legacy_amount_cents / 100).toFixed(2)} off your
                        subscription after your friend's first payment.
                      </p>
                    )}
                    {invite.extended_until && (
                      <p className="text-color-muted">
                        Extended through{" "}
                        {new Date(invite.extended_until).toLocaleDateString()}{" "}
                        when this reward was applied.
                      </p>
                    )}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-color-muted text-sm">
                No accepted invites yet. Copying your link doesn't count as an
                accepted invite.
              </p>
            )}
            {data.invites.some((i) => i.policy === "legacy_payment") && (
              <p className="text-color-muted text-sm">
                Previous referrals keep their original reward terms and count
                toward your 3-invite limit.
              </p>
            )}
          </>
        )}
        <div className="border-border-subtle border-t pt-5 text-sm">
          <p className="font-medium">Missing a referral reward?</p>
          <p className="text-color-muted mt-2">
            If your friend signed up through your link and started their free
            trial, but your free month hasn't appeared,{" "}
            <a href={referralSupportUrl} className="underline">
              email us
            </a>{" "}
            and we'll help.
          </p>
        </div>
      </div>
    </div>
  );
}

function statusLabel(status: ReferralSummary["invites"][number]["status"]) {
  return {
    accepted: "Accepted · Waiting for trial to start",
    applying: "Trial started · Applying your free month",
    applied: "Reward applied · 1 free month added",
    pending: "Reward pending · Contact us for help",
    legacy_pending: "Previous referral offer · Waiting for first payment",
    legacy_applied: "Previous referral offer · Reward applied",
  }[status];
}
