import { Check, Copy } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { getReferralInvites } from "@/functions/referrals";
import { useAnalytics } from "@/hooks/use-posthog";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

const referralInvitesQueryKey = ["referral-invites"];

export function ReferralSection({ ineligible }: { ineligible: boolean }) {
  const session = useAccountSession();
  const { track } = useAnalytics();
  const [copiedSlot, setCopiedSlot] = useState<number | null>(null);
  const isPaid = session.data?.billing.isPaid === true;
  const invitesQuery = useQuery({
    queryKey: referralInvitesQueryKey,
    enabled: typeof window !== "undefined" && isPaid,
    queryFn: () => getReferralInvites(),
  });

  if (session.isPending) {
    return (
      <div className={accountCardClassName}>
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Checking your referral invites...
        </p>
      </div>
    );
  }

  if (!isPaid) {
    return (
      <div className={accountCardClassName}>
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          {ineligible
            ? "Referral invites are for new accounts. Ask your friend to send the link to someone who has not used Anarlog before."
            : "Referral invites unlock when you become a Pro subscriber."}
        </p>
      </div>
    );
  }

  if (invitesQuery.isPending) {
    return (
      <div className={accountCardClassName}>
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          Preparing your three invites...
        </p>
      </div>
    );
  }

  if (invitesQuery.isError || !invitesQuery.data?.length) {
    return (
      <div className={accountCardClassName}>
        <p className="p-6 text-sm leading-6 text-[#756b5d] sm:p-8">
          We could not load your referral invites. Refresh the page to try
          again.
        </p>
      </div>
    );
  }

  const invites = invitesQuery.data;
  const earnedRewards = invites.filter(
    (invite) => invite.status === "reward_earned",
  ).length;
  const rewardAmount = invites[0]?.rewardAmountCents ?? 0;
  const rewardCurrency = invites[0]?.rewardCurrency ?? "usd";
  const availableInvites = invites.filter(
    (invite) => invite.status === "available",
  ).length;
  const earnedCredit = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: rewardCurrency.toUpperCase(),
    maximumFractionDigits: 0,
  }).format((earnedRewards * rewardAmount) / 100);

  const handleCopy = async (slot: number, url: string) => {
    await navigator.clipboard.writeText(url);
    setCopiedSlot(slot);
    track("referral_link_copied", {
      slot,
      available_invites: availableInvites,
    });
    setTimeout(() => setCopiedSlot(null), 2_000);
  };

  return (
    <div className={accountCardClassName}>
      {ineligible && (
        <p className="border-b border-[#ede7dc] bg-[#fffaf0] px-6 py-4 text-sm leading-6 text-[#756b5d] sm:px-8">
          Referral invites are for new accounts. Your own three links are below.
        </p>
      )}
      <ul className="divide-y divide-[#ede7dc]">
        {invites.map((invite) => (
          <li
            key={invite.slot}
            className="flex items-center justify-between gap-4 px-6 py-5 sm:px-8"
          >
            <div>
              <p className="text-base font-medium text-[#181613]">
                Invite {invite.slot}
              </p>
              <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                {statusLabel(invite.status)}
              </p>
            </div>
            <button
              type="button"
              onClick={() => handleCopy(invite.slot, invite.url)}
              className={accountPillSecondaryClassName}
            >
              {copiedSlot === invite.slot ? (
                <>
                  <Check className="mr-2 size-4" />
                  Copied
                </>
              ) : (
                <>
                  <Copy className="mr-2 size-4" />
                  Copy link
                </>
              )}
            </button>
          </li>
        ))}
      </ul>
      <p className="border-t border-[#ede7dc] px-6 py-4 text-sm leading-6 text-[#756b5d] sm:px-8">
        {earnedRewards > 0
          ? `${earnedCredit} in referral credit earned.`
          : "You both get a month of Pro free. Your friend's starts right away, and yours kicks in after their first payment."}
      </p>
    </div>
  );
}

function statusLabel(status: "available" | "trial_started" | "reward_earned") {
  switch (status) {
    case "trial_started":
      return "Trial started";
    case "reward_earned":
      return "Reward earned";
    default:
      return "Available";
  }
}
