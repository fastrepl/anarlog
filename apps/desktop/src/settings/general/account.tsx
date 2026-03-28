import { useMutation, useQuery } from "@tanstack/react-query";
import {
  CheckCircle2,
  Construction,
  Puzzle,
  RefreshCw,
  Sparkle,
  XCircle,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import {
  type ReactNode,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  canStartTrial as canStartTrialApi,
  startTrial,
} from "@hypr/api-client";
import { createClient } from "@hypr/api-client/client";
import { commands as analyticsCommands } from "@hypr/plugin-analytics";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import {
  getActionForTier,
  PLAN_TIERS,
  type PlanTier,
  type TierAction,
} from "@hypr/pricing";
import { Button } from "@hypr/ui/components/ui/button";
import { Input } from "@hypr/ui/components/ui/input";
import { cn } from "@hypr/utils";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing";
import { env } from "~/env";
import { configureProSettings } from "~/shared/config/configure-pro-settings";
import * as settings from "~/store/tinybase/store/settings";

const WEB_APP_BASE_URL = env.VITE_APP_URL ?? "http://localhost:3000";
const ACCOUNT_FEATURES = [
  {
    label: "Cloud Services",
    icon: Sparkle,
    benefit:
      "Get hosted transcription and language models without managing API keys.",
    accent: {
      icon: "text-blue-900",
      label: "text-blue-950",
    },
  },
  {
    label: "Integrations",
    icon: Puzzle,
    benefit: "Connect tools and pull context into Char with less busywork.",
    accent: {
      icon: "text-purple-700",
      label: "text-purple-900",
    },
  },
] as const;

export function SettingsAccount() {
  const auth = useAuth();
  const { plan, isPaid, isPro, isTrialing, trialDaysRemaining } =
    useBillingAccess();

  const isAuthenticated = !!auth?.session;
  const [isPending, setIsPending] = useState(false);
  const [callbackUrl, setCallbackUrl] = useState("");

  useEffect(() => {
    if (isAuthenticated) {
      setIsPending(false);
    }
  }, [isAuthenticated]);

  const handleSignIn = useCallback(async () => {
    setIsPending(true);
    try {
      await auth?.signIn();
    } catch {
      setIsPending(false);
    }
  }, [auth]);

  const signOutMutation = useMutation({
    mutationFn: async () => {
      void analyticsCommands.event({
        event: "user_signed_out",
      });
      void analyticsCommands.setProperties({
        set: {
          is_signed_up: false,
        },
      });

      await auth?.signOut();
    },
  });

  if (!isAuthenticated) {
    if (isPending) {
      return (
        <div className="flex flex-col gap-8">
          <div>
            <h2 className="mb-4 font-serif text-lg font-semibold">Account</h2>
            <Container
              title="Finish sign-in"
              description="Complete the sign-in flow in your browser, then come back here if Char does not reconnect automatically."
              action={
                <Button onClick={handleSignIn} variant="outline">
                  Reopen sign-in page
                </Button>
              }
            >
              <div className="flex flex-col gap-3">
                <p className="text-xs text-neutral-500">
                  Having trouble? Paste the callback URL manually.
                </p>
                <div className="flex flex-col gap-2 sm:flex-row">
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
            </Container>
          </div>
        </div>
      );
    }

    return (
      <div className="flex flex-col gap-8">
        <section className="border-b border-neutral-200 pb-4">
          <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex min-w-0 flex-1 flex-col gap-4">
              <h2 className="font-serif text-lg font-semibold">Account</h2>
              <div className="flex flex-col gap-2">
                <h3 className="text-sm font-medium">Sign in to Char</h3>
                <div className="text-sm text-neutral-600">
                  Sign in to unlock cloud transcription and AI models, plus Pro
                  features like integrations and sharing.
                </div>
              </div>
              <button
                type="button"
                onClick={handleSignIn}
                className="h-10 w-fit rounded-full border-2 border-stone-600 bg-stone-800 px-6 text-sm font-medium text-white shadow-[0_4px_14px_rgba(87,83,78,0.4)] transition-all duration-200 hover:bg-stone-700"
              >
                Get started
              </button>
            </div>
            <div className="shrink-0">
              <FeatureSpotlight />
            </div>
          </div>
        </section>
      </div>
    );
  }

  const currentTier = plan === "trial" ? "pro" : plan;

  return (
    <div className="flex flex-col gap-8">
      <div>
        <h2 className="mb-4 font-serif text-lg font-semibold">Account</h2>
        <Container
          title="Your Account"
          description={auth.session?.user.email ?? "Signed in"}
          action={
            <Button
              variant="outline"
              onClick={() => signOutMutation.mutate()}
              disabled={signOutMutation.isPending}
              className={cn([
                "border-red-200 text-red-700 hover:border-red-300 hover:bg-red-50 hover:text-red-800",
              ])}
            >
              {signOutMutation.isPending ? "Signing out..." : "Sign out"}
            </Button>
          }
        />
      </div>

      <PlanBillingSection
        currentTier={currentTier}
        isTrialing={isTrialing}
        trialDaysRemaining={trialDaysRemaining}
        isPaid={isPaid}
        isPro={isPro}
      />
    </div>
  );
}

function PlanBillingSection({
  currentTier,
  isTrialing,
  trialDaysRemaining,
  isPaid,
  isPro,
}: {
  currentTier: PlanTier;
  isTrialing: boolean;
  trialDaysRemaining: number | null;
  isPaid: boolean;
  isPro: boolean;
}) {
  const auth = useAuth();
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

  const openUrl = useCallback((url: string) => {
    void openerCommands.openUrl(url, null);
  }, []);

  const planLabel =
    currentTier === "free" ? "Free" : currentTier === "lite" ? "Lite" : "Pro";
  const statusText = isTrialing ? (
    <>
      Pro trial
      {trialDaysRemaining != null &&
        ` \u2014 ${trialDaysRemaining} day${trialDaysRemaining === 1 ? "" : "s"} left`}
    </>
  ) : (
    <>
      You&rsquo;re on the <span className="font-semibold">{planLabel}</span>{" "}
      plan
    </>
  );

  const renderAction = (action: TierAction, compact: boolean) => {
    if (action == null) return null;

    if (action.style === "current") {
      if (compact) {
        return <span className="text-xs text-neutral-400">{action.label}</span>;
      }
      return (
        <div className="flex h-8 items-center justify-center rounded-full border border-neutral-200 bg-neutral-50 text-xs text-neutral-500">
          {action.label}
        </div>
      );
    }

    const isUpgrade = action.style === "upgrade";

    const handleClick = () => {
      if (action.label === "Start free trial") {
        startTrialMutation.mutate();
        return;
      }
      if (!action.targetPlan) return;

      void analyticsCommands.event({
        event: "upgrade_clicked",
        plan: action.targetPlan,
      });

      if (isPaid && action.targetPlan) {
        openUrl(
          `${WEB_APP_BASE_URL}/app/switch-plan?targetPlan=${action.targetPlan}&targetPeriod=monthly`,
        );
      } else {
        openUrl(
          `${WEB_APP_BASE_URL}/app/checkout?plan=${action.targetPlan}&period=monthly`,
        );
      }
    };

    const label =
      action.label === "Start free trial" && startTrialMutation.isPending
        ? "Loading..."
        : action.label;

    if (compact) {
      return (
        <button
          type="button"
          onClick={handleClick}
          disabled={
            action.label === "Start free trial" && startTrialMutation.isPending
          }
          className={cn([
            "text-xs font-medium transition-colors",
            isUpgrade
              ? "text-stone-600 hover:text-stone-800"
              : "text-neutral-500 hover:text-neutral-700",
          ])}
        >
          {label}
        </button>
      );
    }

    const buttonClass = cn([
      "flex h-8 w-full cursor-pointer items-center justify-center rounded-full text-xs font-medium transition-all hover:scale-[102%] active:scale-[98%] disabled:opacity-50 disabled:hover:scale-100",
      isUpgrade
        ? "bg-linear-to-t from-stone-600 to-stone-500 text-white shadow-md hover:shadow-lg"
        : "border border-neutral-300 bg-linear-to-b from-white to-stone-50 text-neutral-700 shadow-xs hover:shadow-md",
    ]);

    return (
      <button
        type="button"
        onClick={handleClick}
        disabled={
          action.label === "Start free trial" && startTrialMutation.isPending
        }
        className={buttonClass}
      >
        {label}
      </button>
    );
  };

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h2 className="font-serif text-lg font-semibold">Plan & Billing</h2>
        {isPaid && (
          <button
            type="button"
            onClick={() => openUrl(`${WEB_APP_BASE_URL}/app/account`)}
            className="text-xs text-neutral-500 transition-colors hover:text-neutral-700"
          >
            Manage billing
          </button>
        )}
      </div>

      <div className="mb-4 flex items-center gap-2">
        <p className="text-sm text-neutral-600">{statusText}</p>
        <button
          type="button"
          onClick={() => auth?.refreshSession()}
          className="text-neutral-400 transition-colors hover:text-neutral-600"
          aria-label="Refresh billing status"
        >
          <RefreshCw className="size-3" />
        </button>
      </div>

      <PlanTierList
        currentTier={currentTier}
        isTrialing={isTrialing}
        canStartTrial={canTrialQuery.data ?? false}
        renderAction={renderAction}
      />
    </div>
  );
}

function PlanTierList({
  currentTier,
  isTrialing,
  canStartTrial,
  renderAction,
}: {
  currentTier: PlanTier;
  isTrialing: boolean;
  canStartTrial: boolean;
  renderAction: (action: TierAction, compact: boolean) => ReactNode;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [isWide, setIsWide] = useState(true);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver(([entry]) => {
      setIsWide(entry.contentRect.width >= 480);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={containerRef}>
      {isWide ? (
        <div className="grid grid-cols-3 gap-px border-t border-neutral-100">
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div
                key={tier.id}
                className={cn([
                  "flex flex-col p-3",
                  isCurrent && "bg-stone-50/60",
                ])}
              >
                <div className="mb-2 flex items-center gap-2">
                  <span className="font-serif text-base font-medium text-stone-800">
                    {tier.name}
                  </span>
                  {isCurrent && (
                    <span className="rounded-full bg-stone-600 px-2 py-0.5 text-[10px] font-medium tracking-wide text-white uppercase">
                      {isTrialing ? "Trial" : "Current"}
                    </span>
                  )}
                </div>

                <div className="mb-2">
                  <span className="font-serif text-xl text-stone-700">
                    {tier.price}
                  </span>
                  {tier.period && (
                    <span className="ml-1 text-sm text-neutral-500">
                      {tier.period}
                    </span>
                  )}
                  {tier.subtitle && (
                    <div className="mt-0.5 text-xs text-neutral-400">
                      {tier.subtitle}
                    </div>
                  )}
                </div>

                <div className="mb-3 flex flex-col gap-1">
                  {tier.features.map((feature) => {
                    const Icon =
                      feature.included === true
                        ? CheckCircle2
                        : feature.included === "partial"
                          ? Construction
                          : XCircle;
                    const hoverTitle =
                      feature.included === "partial"
                        ? "Currently in development"
                        : undefined;

                    return (
                      <div
                        key={feature.label}
                        className="flex items-start gap-1.5"
                        title={hoverTitle}
                      >
                        <Icon
                          className={cn([
                            "mt-0.5 size-3.5 shrink-0",
                            feature.included === true
                              ? "text-green-700"
                              : feature.included === "partial"
                                ? "text-yellow-600"
                                : "text-red-500",
                          ])}
                        />
                        <span
                          className={cn([
                            "text-xs",
                            feature.included === false
                              ? "text-neutral-700"
                              : "text-neutral-900",
                          ])}
                        >
                          {feature.label}
                        </span>
                      </div>
                    );
                  })}
                </div>

                <div className="mt-auto">{renderAction(action, false)}</div>
              </div>
            );
          })}
        </div>
      ) : (
        <div className="flex flex-col">
          {PLAN_TIERS.map((tier) => {
            const isCurrent = tier.id === currentTier;
            const action = getActionForTier(
              tier.id,
              currentTier,
              canStartTrial,
            );

            return (
              <div
                key={tier.id}
                className={cn([
                  "flex items-center justify-between border-b border-neutral-100 py-2.5 last:border-b-0",
                  isCurrent && "-mx-2 rounded-md bg-stone-50/60 px-2",
                ])}
              >
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-stone-800">
                    {tier.name}
                  </span>
                  <span className="text-sm text-neutral-500">
                    {tier.price}
                    {tier.period}
                  </span>
                  {isCurrent && (
                    <span className="rounded-full bg-stone-600 px-1.5 py-px text-[10px] font-medium tracking-wide text-white uppercase">
                      {isTrialing ? "Trial" : "Current"}
                    </span>
                  )}
                </div>
                <div className="shrink-0">{renderAction(action, true)}</div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function FeatureSpotlight() {
  const [activeIndex, setActiveIndex] = useState(0);
  const [isPaused, setIsPaused] = useState(false);

  useEffect(() => {
    if (isPaused) {
      return;
    }

    const interval = window.setInterval(() => {
      setActiveIndex((current) => (current + 1) % ACCOUNT_FEATURES.length);
    }, 2200);

    return () => window.clearInterval(interval);
  }, [isPaused]);

  const { label, icon: Icon, benefit, accent } = ACCOUNT_FEATURES[activeIndex];

  return (
    <div className="group relative flex w-full max-w-[220px] min-w-[180px] items-center justify-center p-2">
      <div className="relative min-h-[88px] w-full">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={label}
            initial={{ opacity: 0, y: 10, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -10, scale: 0.96 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
            className="absolute inset-0"
          >
            <motion.button
              type="button"
              initial={{ opacity: 0, y: 10, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -10, scale: 0.96 }}
              transition={{ duration: 0.22, ease: "easeOut" }}
              onMouseEnter={() => setIsPaused(true)}
              onMouseLeave={() => setIsPaused(false)}
              onFocus={() => setIsPaused(true)}
              onBlur={() => setIsPaused(false)}
              className="absolute inset-0 flex flex-col items-center justify-center gap-3 text-center outline-none"
              aria-label={`${label}. ${benefit}`}
            >
              <motion.div
                initial={{ scale: 0.86, rotate: -10 }}
                animate={{
                  scale: isPaused ? 1.08 : 1,
                  rotate: 0,
                  y: isPaused ? -2 : 0,
                }}
                exit={{ scale: 0.9, rotate: 10 }}
                transition={{ duration: 0.28, ease: "easeOut" }}
                className="flex h-12 w-12 items-center justify-center"
              >
                <motion.div
                  animate={
                    isPaused ? { rotate: [0, -4, 4, 0] } : { y: [0, -2, 0] }
                  }
                  transition={{
                    duration: isPaused ? 0.9 : 1.6,
                    repeat: Number.POSITIVE_INFINITY,
                    ease: "easeInOut",
                  }}
                >
                  <Icon className={cn(["h-5 w-5", accent.icon])} />
                </motion.div>
              </motion.div>
              <p className={cn(["text-sm font-medium", accent.label])}>
                {label}
              </p>
            </motion.button>
          </motion.div>
        </AnimatePresence>
      </div>
      <AnimatePresence>
        {isPaused ? (
          <motion.div
            initial={{ opacity: 0, y: 8, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 6, scale: 0.98 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="pointer-events-none absolute top-full right-0 z-10 mt-1.5 w-[208px] rounded-xl border border-neutral-200 bg-white/95 p-2.5 text-left shadow-lg backdrop-blur-sm"
          >
            <div className="flex items-center justify-between gap-3">
              <p className={cn(["text-sm font-medium", accent.label])}>
                {label}
              </p>
            </div>
            <p className="mt-1 text-xs leading-[1.45] text-neutral-600">
              {benefit}
            </p>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
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
    <section className="border-b border-neutral-200 pb-4 last:border-b-0">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="flex min-w-0 flex-1 flex-col gap-2">
          <h3 className="text-sm font-medium">{title}</h3>
          {description && (
            <div className="text-sm text-neutral-600">{description}</div>
          )}
        </div>
        {action ? <div className="shrink-0">{action}</div> : null}
      </div>
      {children ? <div className="mt-4">{children}</div> : null}
    </section>
  );
}
