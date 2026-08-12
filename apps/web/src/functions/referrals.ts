import type { SupabaseClient } from "@supabase/supabase-js";
import { createServerFn, createServerOnlyFn } from "@tanstack/react-start";
import {
  deleteCookie,
  getCookie,
  setCookie,
  setResponseHeader,
} from "@tanstack/react-start/server";
import { z } from "zod";

import { getRequestAppOrigin } from "@/functions/app-origin";
import { getSupabaseServerClient } from "@/functions/supabase";

const REFERRAL_COOKIE = "anarlog-referral";
const REFERRAL_COOKIE_MAX_AGE_SECONDS = 7 * 24 * 60 * 60;
const referralCodeSchema = z.string().regex(/^[a-f0-9]{24}$/);

type ReferralInviteRow = {
  slot: number;
  code: string;
  status: "available" | "trial_started" | "reward_earned";
  reward_amount_cents: number;
  reward_currency: string;
};

export const persistReferralAttribution = createServerFn({ method: "POST" })
  .inputValidator(referralCodeSchema)
  .handler(async ({ data: code }) => {
    setPrivateResponseHeaders();
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();

    if (user) {
      deleteCookie(REFERRAL_COOKIE, { path: "/" });
      return "existing_account" as const;
    }

    setCookie(REFERRAL_COOKIE, code, {
      httpOnly: true,
      maxAge: REFERRAL_COOKIE_MAX_AGE_SECONDS,
      path: "/",
      sameSite: "lax",
      secure: getRequestAppOrigin().startsWith("https://"),
    });
    return "stored" as const;
  });

export const claimPendingReferral = createServerOnlyFn(
  async (supabase: SupabaseClient) => {
    const rawCode = getCookie(REFERRAL_COOKIE);
    const parsedCode = referralCodeSchema.safeParse(rawCode);
    if (!parsedCode.success) {
      if (rawCode) {
        deleteCookie(REFERRAL_COOKIE, { path: "/" });
      }
      return false;
    }

    const { data, error } = await supabase.rpc("claim_referral", {
      p_code: parsedCode.data,
    });
    if (error) {
      throw error;
    }

    deleteCookie(REFERRAL_COOKIE, { path: "/" });
    return data === true;
  },
);

export const getReferralTrialDays = createServerOnlyFn(
  async (supabase: SupabaseClient) => {
    const { data, error } = await supabase.rpc("get_referral_trial_days");
    if (error) {
      throw error;
    }
    return typeof data === "number" ? data : null;
  },
);

export const getReferralInvites = createServerFn({ method: "POST" }).handler(
  async () => {
    setPrivateResponseHeaders();
    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();
    if (!user?.id || user.is_anonymous) {
      throw new Error("Unauthorized");
    }

    const { data, error } = await supabase.rpc(
      "get_or_create_referral_invites",
    );
    if (error) {
      throw error;
    }

    const appOrigin = getRequestAppOrigin();
    return ((data ?? []) as ReferralInviteRow[]).map((invite) => ({
      slot: invite.slot,
      status: invite.status,
      rewardAmountCents: invite.reward_amount_cents,
      rewardCurrency: invite.reward_currency,
      url: `${appOrigin}/invite/${invite.code}`,
    }));
  },
);

function setPrivateResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}
