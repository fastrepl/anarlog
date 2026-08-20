import { createServerFn } from "@tanstack/react-start";
import { createHash } from "node:crypto";

import { env, requireEnv } from "@/env";
import { getStripeCustomerIdForUser } from "@/functions/billing";
import { getStripeClient } from "@/functions/stripe";
import {
  getSupabaseAdminClient,
  getSupabaseServerClient,
} from "@/functions/supabase";
import { sendLoopsEvent, sendLoopsTransactional } from "@/lib/loops";
import {
  normalizeYcVerificationUrl,
  parseYcPerkApplyValue,
  verifyYcFounder,
  ycPerkApplyInputSchema,
  ycPerkRequestSchema,
} from "@/lib/yc-perk";
import {
  applyYcPromotionToCustomer,
  findYcPromotionCodeByCustomerCode,
  isYcPromotionAvailable,
} from "@/lib/yc-perk-apply";
import {
  createYcPromotionCode,
  getOrCreateYcPromotionCode,
  getYcPerkClaimId,
} from "@/lib/yc-perk-promotion";

const YC_PERK_TRANSACTIONAL_ID = "cmshjqnof011b0jxiks475igx";

async function getOrCreateYcPerkClaimCode(claimId: string) {
  const supabase = getSupabaseAdminClient();
  const candidateCode = createYcPromotionCode();
  const { error: insertError } = await supabase
    .from("yc_perk_claims")
    .upsert(
      { claim_id: claimId, promotion_code: candidateCode },
      { onConflict: "claim_id", ignoreDuplicates: true },
    );
  if (insertError) {
    throw insertError;
  }

  const { data: claim, error: claimError } = await supabase
    .from("yc_perk_claims")
    .select("promotion_code")
    .eq("claim_id", claimId)
    .single();
  if (claimError) {
    throw claimError;
  }

  return claim.promotion_code as string;
}

export const submitYcPerkRequest = createServerFn({ method: "POST" })
  .inputValidator(ycPerkRequestSchema)
  .handler(async ({ data }) => {
    if (data.additionalComments) {
      return { status: "submitted" as const };
    }

    const verificationUrl = normalizeYcVerificationUrl(data.verificationUrl);
    const verification = await verifyYcFounder({ verificationUrl });
    if (verification.status === "invalid") {
      return verification;
    }

    const email = verification.email;
    const claimId = getYcPerkClaimId(email);
    const requestId = createHash("sha256")
      .update(`${email}\n${verificationUrl}`)
      .digest("hex");

    const promotionCode = await getOrCreateYcPerkClaimCode(claimId);
    const promotion = await getOrCreateYcPromotionCode({
      stripe: getStripeClient(),
      claimId,
      code: promotionCode,
    });
    if (promotion.status === "claimed") {
      return { status: "already_claimed" as const };
    }

    const loopsKey = requireEnv(env.LOOPS_KEY, "LOOPS_KEY");
    await sendLoopsTransactional({
      apiKey: loopsKey,
      transactionalId: YC_PERK_TRANSACTIONAL_ID,
      email,
      dataVariables: {
        firstName: verification.firstName,
        promotionCode: promotion.code,
      },
      idempotencyKey: `yc-perk-email:${requestId}`,
    });
    try {
      await sendLoopsEvent({
        apiKey: loopsKey,
        email,
        eventName: "anarlogYcPerkRequested",
        firstName: verification.firstName,
        eventProperties: {
          source: "yc_perk_page",
          verificationUrl,
        },
        idempotencyKey: `yc-perk-event:${requestId}`,
      });
    } catch (error) {
      console.error("Failed to record YC perk request:", error);
    }

    return { status: "verified" as const };
  });

export const applyYcPerk = createServerFn({ method: "POST" })
  .inputValidator(ycPerkApplyInputSchema)
  .handler(async ({ data }) => {
    const parsed = parseYcPerkApplyValue(data.value);
    if (parsed.type === "invalid") {
      return { status: "invalid_input" as const, message: parsed.message };
    }

    const supabase = getSupabaseServerClient();
    const {
      data: { user },
    } = await supabase.auth.getUser();
    if (!user?.id || user.is_anonymous) {
      throw new Error("Unauthorized");
    }

    const stripe = getStripeClient();
    let promotion: {
      id: string;
      code: string;
      active: boolean;
      max_redemptions: number | null;
      times_redeemed: number;
    } | null = null;

    if (parsed.type === "verification_url") {
      const verification = await verifyYcFounder({
        verificationUrl: parsed.verificationUrl,
      });
      if (verification.status === "invalid") {
        return verification;
      }

      const claimId = getYcPerkClaimId(verification.email);
      const promotionCode = await getOrCreateYcPerkClaimCode(claimId);
      promotion = await getOrCreateYcPromotionCode({
        stripe,
        claimId,
        code: promotionCode,
      });
    } else {
      promotion = await findYcPromotionCodeByCustomerCode(stripe, parsed.code);
      if (!promotion) {
        return { status: "invalid_code" as const };
      }
    }

    const stripeCustomerId = await getStripeCustomerIdForUser(
      supabase,
      stripe,
      user,
    );
    if (!stripeCustomerId) {
      if (!isYcPromotionAvailable(promotion)) {
        return { status: "claimed" as const };
      }
      return { status: "needs_checkout" as const, code: promotion.code };
    }

    const result = await applyYcPromotionToCustomer({
      stripe,
      customerId: stripeCustomerId,
      promotion,
    });
    if (result.status === "needs_checkout") {
      return { status: "needs_checkout" as const, code: promotion.code };
    }
    return result;
  });
