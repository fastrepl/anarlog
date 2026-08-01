import type { AuthChangeEvent, Session } from "@supabase/supabase-js";
import { getVersion } from "@tauri-apps/api/app";
import { version as osVersion, platform } from "@tauri-apps/plugin-os";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as authPluginCommands } from "@anlg/plugin-auth";
import { deriveBillingInfo } from "@anlg/supabase";

import { setErrorReportingUser } from "~/error-reporting";

let trackedIdentifySignature: string | null = null;
let trackedSignedInUserId: string | null = null;

export function resetTrackedAuthIdentity() {
  trackedIdentifySignature = null;
  trackedSignedInUserId = null;
  setErrorReportingUser(null);
}

export async function clearAuthAnalyticsGroups() {
  await analyticsCommands.clearGroups();
}

async function getBillingAnalytics(accessToken: string) {
  const result = await authPluginCommands.decodeClaims(accessToken);
  if (result.status === "error") {
    return {
      plan: "free" as const,
      trialEndDate: null,
    };
  }

  const billing = deriveBillingInfo({
    sub: result.data.sub,
    email: result.data.email ?? undefined,
    entitlements: result.data.entitlements,
    subscription_status: result.data.subscription_status,
    trial_end: result.data.trial_end,
  });

  return {
    plan: billing.plan,
    trialEndDate: billing.trialEnd?.toISOString() ?? null,
  };
}

export async function trackAuthEvent(
  event: AuthChangeEvent,
  session: Session | null,
): Promise<void> {
  if (
    (event === "SIGNED_IN" ||
      event === "INITIAL_SESSION" ||
      event === "TOKEN_REFRESHED") &&
    session
  ) {
    setErrorReportingUser(session.user.id);
    const appVersion = await getVersion();
    const billing = await getBillingAnalytics(session.access_token);
    const identifySignature = JSON.stringify({
      userId: session.user.id,
      email: session.user.email ?? null,
      plan: billing.plan,
      trialEndDate: billing.trialEndDate,
      appVersion,
    });

    if (identifySignature !== trackedIdentifySignature) {
      trackedIdentifySignature = identifySignature;

      await analyticsCommands.identify(session.user.id, {
        email: session.user.email,
        set: {
          account_created_date: session.user.created_at,
          is_signed_up: true,
          app_version: appVersion,
          os_version: osVersion(),
          platform: platform(),
          plan: billing.plan,
          trial_end_date: billing.trialEndDate,
        },
        group: {
          type: "account",
          key: session.user.id,
          properties: {
            name: session.user.email ?? session.user.id,
            email: session.user.email ?? null,
            created_at: session.user.created_at,
            plan: billing.plan,
            trial_end_date: billing.trialEndDate,
          },
        },
      });
    }

    if (event === "SIGNED_IN" && trackedSignedInUserId !== session.user.id) {
      trackedSignedInUserId = session.user.id;
      await analyticsCommands.event({ event: "user_signed_in" });
    }
  }
}
