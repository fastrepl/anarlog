export type ReferralSummary = {
  enabled: boolean;
  eligible: boolean;
  code: string | null;
  accepted: number;
  remaining: number;
  months_earned: number;
  invites: {
    id: string;
    slot: number;
    policy: "legacy_payment" | "trial_month";
    accepted_at: string;
    trial_started_at: string | null;
    reward_applied_at: string | null;
    extended_until: string | null;
    legacy_amount_cents: number;
    status:
      | "accepted"
      | "applying"
      | "applied"
      | "pending"
      | "legacy_pending"
      | "legacy_applied";
  }[];
};

export const referralSupportUrl =
  "mailto:team@fastrepl.com?subject=Referral%20reward%20help";
