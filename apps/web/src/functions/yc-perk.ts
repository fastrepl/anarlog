import { createServerFn } from "@tanstack/react-start";
import { createHash } from "node:crypto";

import { env, requireEnv } from "@/env";
import { getStripeClient } from "@/functions/stripe";
import { getSupabaseAdminClient } from "@/functions/supabase";
import { sendLoopsEvent, sendLoopsTransactional } from "@/lib/loops";
import {
  normalizeYcVerificationUrl,
  verifyYcFounder,
  ycPerkRequestSchema,
} from "@/lib/yc-perk";
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

    const email = data.email.trim().toLowerCase();
    const verificationUrl = normalizeYcVerificationUrl(data.verificationUrl);
    const claimId = getYcPerkClaimId(email);
    const requestId = createHash("sha256")
      .update(`${email}\n${verificationUrl}`)
      .digest("hex");

    const verification = await verifyYcFounder({ email, verificationUrl });
    if (verification.status === "invalid") {
      return verification;
    }

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
