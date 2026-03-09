import { useMutation, useQuery } from "@tanstack/react-query";
import {
  Brain,
  Cloud,
  ExternalLinkIcon,
  Puzzle,
  Sparkle,
  Sparkles,
} from "lucide-react";
import { type ReactNode, useCallback, useEffect, useState } from "react";

import {
  canStartTrial as canStartTrialApi,
  startTrial,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { type SubscriptionStatus } from "@hypr/supabase";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { Spinner } from "@hypr/ui/components/ui/spinner";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { configureProSettings } from "~/shared/config/configure-pro-settings";
import * as settings from "~/store/tinybase/store/settings";

const WEB_APP_BASE_URL = env.VITE_APP_URL ?? "http://localhost:3000";

function PlanStatus({
  subscriptionStatus,
  trialDaysRemaining,
}: {
  subscriptionStatus: SubscriptionStatus | null;
  trialDaysRemaining: number | null;
}) {
  if (!subscriptionStatus) {
    return <span className="text-neutral-500">FREE</span>;
  }

  switch (subscriptionStatus) {
    case "active":
      return (
        <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
          <Sparkles size={13} className="text-neutral-500" />
          PRO
        </span>
      );

    case "trialing": {
      const isUrgent = trialDaysRemaining !== null && trialDaysRemaining <= 3;
      let trialText = null;
      if (trialDaysRemaining !== null) {
        if (trialDaysRemaining === 0) {
          trialText = "Trial ends today";
        } else if (trialDaysRemaining === 1) {
          trialText = "Trial ends tomorrow";
        } else {
          trialText = `${trialDaysRemaining} days left`;
        }
      }
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
            <Sparkles size={13} className="text-neutral-500" />
            PRO
          </span>
          {trialText && (
            <span
              className={cn(["text-neutral-500", isUrgent && "text-amber-600"])}
            >
              ({trialText})
            </span>
          )}
        </span>
      );
    }

    case "past_due":
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 font-medium text-neutral-800">
            <Sparkles size={13} className="text-neutral-500" />
            PRO
          </span>
          <span className="text-amber-600">(Payment issue)</span>
        </span>
      );

    case "unpaid":
      return <span className="text-amber-600">Payment failed</span>;

    case "canceled":
      return <span className="text-neutral-500">Canceled</span>;

    case "incomplete":
      return <span className="text-neutral-500">Setup incomplete</span>;

    case "incomplete_expired":
      return <span className="text-neutral-500">Expired</span>;

    case "paused":
      return <span className="text-neutral-500">Paused</span>;

    default:
      return <span className="text-neutral-500">FREE</span>;
  }
}

export function AccountSettings() {
  const auth = useAuth();
  const { subscriptionStatus, trialDaysRemaining } = useBillingAccess();

  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);
  const [callbackUrl, setCallbackUrl] = useState("");

  useEffect(() => {
    if (isAuthenticated) {
      setIsPending(false);
    }
  }, [isAuthenticated]);

  const handleOpenAccount = useCallback(() => {
    void openerCommands.openUrl(`${WEB_APP_BASE_URL}/app/account`, null);
  }, []);

  const handleSignIn = useCallback(async () => {
    setIsPending(true);
    try {
      await auth?.signIn();
    } catch {
      setIsPending(false);
    }
  }, [auth]);

  const handleSignOut = useCallback(async () => {
    void analyticsCommands.event({
      event: "user_signed_out",
    });
    void analyticsCommands.setProperties({
      set: {
        is_signed_up: false,
      },
    });

    await auth?.signOut();
  }, [auth]);

  const handleRefreshPlan = useCallback(async () => {
    await auth?.refreshSession();
  }, [auth]);

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div className="flex flex-col items-center gap-6 text-center">
          <div className="flex flex-col gap-2">
            <h2 className="font-serif text-2xl font-semibold">
              Waiting for sign-in...
            </h2>
            <p className="text-base text-neutral-500">
              Complete the sign-in process in your browser
            </p>
          </div>
          <div className="flex w-full max-w-xs flex-col gap-2">
            <Button onClick={handleSignIn} variant="outline" className="w-full">
              Reopen sign-in page
            </Button>
            <div className="flex w-full items-center gap-2">
              <div className="flex-1 border-t border-neutral-200" />
              <span className="shrink-0 text-xs text-neutral-400">
                Having trouble?
              </span>
              <div className="flex-1 border-t border-neutral-200" />
            </div>
            <div className="flex w-full items-center gap-2">
              <Input
                type="text"
                className="flex-1 font-mono text-xs"
                placeholder="hyprnote://deeplink/auth?access_token=..."
                value={callbackUrl}
                onChange={(e) => setCallbackUrl(e.target.value)}
              />
              <Button
                onClick={() => auth?.handleAuthCallback(callbackUrl)}
                disabled={!callbackUrl}
              >
                Submit
              </Button>
            </div>
          </div>
        </div>
      );
    }

    return (
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="flex flex-col gap-2">
          <h2 className="font-serif text-2xl font-semibold">Sign in to Char</h2>
          <p className="text-base text-neutral-500">
            Get started without an account. Sign in to unlock more.
          </p>
        </div>

        <button
          onClick={handleSignIn}
          className="h-10 rounded-full border-2 border-stone-600 bg-stone-800 px-6 text-sm font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 hover:bg-stone-700"
        >
          Get Started
        </button>

        <div className="scrollbar-hide mt-4 flex gap-3 overflow-x-auto">
          {[
            { label: "Pro AI models", icon: Sparkle, comingSoon: false },
            { label: "Cloud sync", icon: Cloud, comingSoon: true },
            { label: "Memory", icon: Brain, comingSoon: true },
            { label: "Integrations", icon: Puzzle, comingSoon: true },
          ].map(({ label, icon: Icon, comingSoon }) => (
            <div
              key={label}
              className="relative flex h-20 w-20 shrink-0 flex-col items-center justify-center gap-2 overflow-hidden rounded-lg border border-neutral-200 bg-linear-to-b from-white to-stone-50 text-neutral-600"
            >
              {comingSoon && (
                <span className="absolute top-0 rounded-b bg-neutral-200 px-1.5 py-0.5 text-[10px] text-neutral-500 opacity-50">
                  Soon
                </span>
              )}
              <Icon className="h-5 w-5" />
              <span className="text-center text-xs leading-tight">{label}</span>
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <Container
        title="Your Account"
        description="Redirect to the web app to manage your account."
        action={
          <div className="flex flex-row gap-2">
            <Button
              variant="outline"
              onClick={handleOpenAccount}
              className="flex w-[100px] flex-row gap-1.5"
            >
              <span className="text-sm">Open</span>
              <ExternalLinkIcon className="text-neutral-600" size={12} />
            </Button>
            <Button variant="outline" onClick={handleSignOut}>
              Sign out
            </Button>
          </div>
        }
      ></Container>

      <Container
        title="Plan & Billing"
        description={
          <span>
            Your current plan is{" "}
            <PlanStatus
              subscriptionStatus={subscriptionStatus}
              trialDaysRemaining={trialDaysRemaining}
            />
          </span>
        }
        action={<BillingButton />}
      >
        <div className="flex items-center gap-1 text-sm text-neutral-600">
          {auth?.isRefreshingSession ? (
            <>
              <Spinner size={14} />
              <span>Refreshing plan status...</span>
            </>
          ) : (
            <>
              Click{" "}
              <span
                onClick={handleRefreshPlan}
                className="text-primary cursor-pointer underline"
              >
                here
              </span>
              <span className="text-neutral-600"> to refresh plan status.</span>
            </>
          )}
        </div>
      </Container>
    </div>
  );
}

function BillingButton() {
  const auth = useAuth();
  const { isPro } = useBillingAccess();
  const store = settings.UI.useStore(settings.STORE_ID);

  const canTrialQuery = useQuery({
    enabled: !!auth?.session && !isPro,
    queryKey: [auth?.session?.user.id ?? "", "canStartTrial"],
    queryFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        return false;
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { data, error } = await canStartTrialApi({ client });
      if (error) {
        throw error;
      }

      return data?.canStartTrial ?? false;
    },
  });

  const startTrialMutation = useMutation({
    mutationFn: async () => {
      const headers = auth?.getHeaders();
      if (!headers) {
        throw new Error("Not authenticated");
      }
      const client = createClient({ baseUrl: env.VITE_API_URL, headers });
      const { error } = await startTrial({
        client,
        query: { interval: "monthly" },
      });
      if (error) {
        throw error;
      }

      await new Promise((resolve) => setTimeout(resolve, 3000));
    },
    onSuccess: async () => {
      if (store) {
        configureProSettings(store);
      }
      await auth?.refreshSession();
    },
  });

  const handleProUpgrade = useCallback(() => {
    void analyticsCommands.event({
      event: "upgrade_clicked",
      plan: "pro",
    });
    void openerCommands.openUrl(
      `${WEB_APP_BASE_URL}/app/checkout?period=monthly`,
      null,
    );
  }, []);

  const handleOpenAccount = useCallback(() => {
    void openerCommands.openUrl(`${WEB_APP_BASE_URL}/app/account`, null);
  }, []);

  if (isPro) {
    return (
      <Button
        variant="outline"
        onClick={handleOpenAccount}
        className="flex w-[100px] flex-row gap-1.5"
      >
        <span className="text-sm">Manage</span>
        <ExternalLinkIcon className="text-neutral-600" size={12} />
      </Button>
    );
  }

  if (canTrialQuery.data) {
    return (
      <Button
        variant="outline"
        onClick={() => startTrialMutation.mutate()}
        disabled={startTrialMutation.isPending}
      >
        <span> Start Pro Trial</span>
      </Button>
    );
  }

  return (
    <Button variant="outline" onClick={handleProUpgrade}>
      <span>Upgrade to Pro</span>
      <ExternalLinkIcon className="text-neutral-600" size={12} />
    </Button>
  );
}

function Container({
  title,
  description,
  action,
  children,
}: {
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-4 rounded-lg bg-neutral-50 p-4">
      <div className="flex flex-col gap-2">
        <h1 className="text-md font-serif font-semibold">{title}</h1>
        {description && (
          <p className="text-sm text-neutral-600">{description}</p>
        )}
      </div>
      {action ? <div>{action}</div> : null}
      {children}
    </section>
  );
}
