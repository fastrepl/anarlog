import { Link } from "@tanstack/react-router";

import { useAccountSession } from "./-account-session";
import {
  accountCardClassName,
  accountPillPrimaryClassName,
  accountPillSecondaryClassName,
} from "./-account-ui";

export function PlanSection() {
  const { data, isPending } = useAccountSession();
  const billing = data?.billing;

  const planLabel = billing?.isTrialing
    ? "Pro trial"
    : billing?.isPaid
      ? billing.isLite && !billing.isPro
        ? "Lite"
        : "Pro"
      : "Free";

  const planDetail = billing?.isTrialing
    ? billing.trialEnd
      ? `${billing.trialDaysRemaining} ${
          billing.trialDaysRemaining === 1 ? "day" : "days"
        } left · ends ${billing.trialEnd.toLocaleDateString("en-US", {
          month: "long",
          day: "numeric",
        })}.`
      : "Your trial is running."
    : billing?.isPaid
      ? "Thanks for supporting Anarlog."
      : "On-device basics, free forever.";

  return (
    <div className={accountCardClassName}>
      <div className="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between sm:p-8">
        {isPending ? (
          <p className="text-sm leading-6 text-[#756b5d]">
            Checking your plan...
          </p>
        ) : (
          <>
            <div>
              <p className="text-base font-medium text-[#181613]">
                You're on{" "}
                <mark className="bg-[#fff0b3] px-1 font-semibold">
                  {planLabel}
                </mark>
              </p>
              <p className="mt-1 text-sm leading-6 text-[#756b5d]">
                {planDetail}
              </p>
            </div>
            {billing?.isPaid || billing?.isTrialing ? (
              <Link to="/app/portal/" className={accountPillSecondaryClassName}>
                Manage billing
              </Link>
            ) : (
              <Link
                to="/app/checkout/"
                search={{
                  period: "monthly",
                  trial: "false",
                  source: "settings",
                }}
                className={accountPillPrimaryClassName}
              >
                Upgrade to Pro
              </Link>
            )}
          </>
        )}
      </div>
    </div>
  );
}
