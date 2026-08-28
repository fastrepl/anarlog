import { Check, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { getReferralInvites } from "@/functions/referrals";
import { useAnalytics } from "@/hooks/use-posthog";

import { useAccountSession } from "./-account-session";
import { accountStyles } from "./-account-ui";
const styles = stylex.create({
  style1: {
    padding: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style2: {
    borderBottomStyle: "solid",
    borderBottomWidth: "1px",
    borderColor: "#ede7dc",
    backgroundColor: "#fffaf0",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style3: {
    borderBottomColor: {
      ":is(*) > :not(:last-child)": "#ede7dc",
    },
    borderBottomStyle: {
      ":is(*) > :not(:last-child)": "solid",
    },
    borderBottomWidth: {
      ":is(*) > :not(:last-child)": "1px",
    },
  },
  style4: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "1rem",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1.25rem",
  },
  style5: {
    fontSize: "1rem",
    lineHeight: "1.5rem",
    fontWeight: 500,
    color: "#181613",
  },
  style6: {
    marginTop: ".25rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
  style7: {
    marginRight: ".5rem",
    width: "1rem",
    height: "1rem",
  },
  style8: {
    borderTopStyle: "solid",
    borderTopWidth: "1px",
    borderColor: "#ede7dc",
    paddingInline: {
      default: "1.5rem",
      "@media (width >= 40rem)": "2rem",
    },
    paddingBlock: "1rem",
    fontSize: ".875rem",
    lineHeight: "1.5rem",
    color: "#756b5d",
  },
});
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
      <div {...stylex.props(accountStyles.card)}>
        <p {...stylex.props(styles.style1)}>
          Checking your referral invites...
        </p>
      </div>
    );
  }
  if (!isPaid) {
    return (
      <div {...stylex.props(accountStyles.card)}>
        <p {...stylex.props(styles.style1)}>
          {ineligible
            ? "Referral invites are for new accounts. Ask your friend to send the link to someone who has not used Anarlog before."
            : "Referral invites unlock when you become a Pro subscriber."}
        </p>
      </div>
    );
  }
  if (invitesQuery.isPending) {
    return (
      <div {...stylex.props(accountStyles.card)}>
        <p {...stylex.props(styles.style1)}>Preparing your three invites...</p>
      </div>
    );
  }
  if (invitesQuery.isError || !invitesQuery.data?.length) {
    return (
      <div {...stylex.props(accountStyles.card)}>
        <p {...stylex.props(styles.style1)}>
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
    <div {...stylex.props(accountStyles.card)}>
      {ineligible && (
        <p {...stylex.props(styles.style2)}>
          Referral invites are for new accounts. Your own three links are below.
        </p>
      )}
      <ul {...stylex.props(styles.style3)}>
        {invites.map((invite) => (
          <li key={invite.slot} {...stylex.props(styles.style4)}>
            <div>
              <p {...stylex.props(styles.style5)}>Invite {invite.slot}</p>
              <p {...stylex.props(styles.style6)}>
                {statusLabel(invite.status)}
              </p>
            </div>
            <button
              type="button"
              onClick={() => handleCopy(invite.slot, invite.url)}
              {...stylex.props([
                accountStyles.pill,
                accountStyles.pillSecondary,
              ])}
            >
              {copiedSlot === invite.slot ? (
                <>
                  <Check {...stylex.props(styles.style7)} />
                  Copied
                </>
              ) : (
                <>
                  <Copy {...stylex.props(styles.style7)} />
                  Copy link
                </>
              )}
            </button>
          </li>
        ))}
      </ul>
      <p {...stylex.props(styles.style8)}>
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
