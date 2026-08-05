import { Link } from "@tanstack/react-router";

import {
  MARKETING_PLAN_TIERS,
  type MarketingPlanData,
  PlanFeatureList,
} from "@anlg/pricing";
import { cn } from "@anlg/utils";

export function PricingSection() {
  return (
    <section id="pricing" className="pt-24 pb-8 md:pt-28 md:pb-10">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          Simple pricing
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          Start with local meeting notes for free. Upgrade when you want hosted
          transcription, AI, sync, and sharing.
        </p>
      </div>

      <div className="relative left-1/2 mt-8 grid w-screen max-w-[760px] -translate-x-1/2 grid-cols-1 gap-4 px-5 text-left md:grid-cols-2 md:px-8">
        {MARKETING_PLAN_TIERS.map((plan) => (
          <PricingCard key={plan.id} plan={plan} />
        ))}
      </div>
    </section>
  );
}

function PricingCard({ plan }: { plan: MarketingPlanData }) {
  const visibleFeatures = plan.features.filter(
    (feature) => feature.included === true,
  );

  return (
    <article
      className={cn([
        "flex min-h-[30rem] flex-col rounded-[3px] border bg-white p-6 transition-all duration-200",
        plan.popular
          ? "border-[#181613]/30 shadow-[0_22px_60px_rgba(24,22,19,0.14)] ring-1 ring-[#181613]/10"
          : "border-neutral-200 opacity-[0.58] shadow-[0_10px_32px_rgba(24,22,19,0.05)] focus-within:opacity-100 focus-within:shadow-[0_16px_46px_rgba(24,22,19,0.08)] hover:opacity-100 hover:shadow-[0_16px_46px_rgba(24,22,19,0.08)]",
      ])}
    >
      <div className="flex items-start">
        <h3 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
          {plan.name}
        </h3>
      </div>

      <p className="mt-4 min-h-[4.5rem] text-sm leading-6 text-[#4f4940]">
        {plan.description}
      </p>

      <div className="mt-5 min-h-[4rem]">
        {plan.price ? (
          <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
            <span className="font-hand text-5xl leading-none font-semibold text-[#181613]">
              ${plan.price.monthly}
            </span>
            <span className="text-sm text-[#756b5d]">/month</span>
            {plan.price.yearly != null ? (
              <span className="text-sm text-[#756b5d]">
                or ${plan.price.yearly}/year
              </span>
            ) : null}
          </div>
        ) : (
          <div className="flex items-baseline gap-2">
            <span className="font-hand text-5xl leading-none font-semibold text-[#181613]">
              $0
            </span>
            <span className="text-sm text-[#756b5d]">/month</span>
          </div>
        )}
      </div>

      <div className="mt-5">
        <PlanFeatureList features={visibleFeatures} dense />
      </div>

      <div className="mt-auto pt-6">
        <Link
          to="/download/"
          className={cn([
            "flex h-11 w-full items-center justify-center rounded-full text-sm font-medium transition-all hover:scale-[102%] active:scale-[98%]",
            plan.popular
              ? "bg-[#181613] text-white hover:bg-[#4f4940]"
              : "bg-[#f4efe6] text-[#181613] hover:bg-[#eadfce]",
          ])}
        >
          {plan.price ? "Start your 3-week Pro trial" : "Download for free"}
        </Link>
      </div>
    </article>
  );
}
