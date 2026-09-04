import { Link } from "@tanstack/react-router";

import {
  MARKETING_PLAN_TIERS,
  type MarketingPlanData,
  type MarketingPlanPrice,
  PlanFeatureList,
} from "@anlg/pricing";
import { cn } from "@anlg/utils";

export function PricingSection({
  compareLink = false,
}: {
  compareLink?: boolean;
}) {
  return (
    <section id="pricing" className="pt-24 pb-8 md:pt-28 md:pb-10">
      <div>
        <h2 className="text-color-muted font-mono text-3xl leading-none font-semibold">
          Simple pricing
        </h2>
        <p className="text-color-secondary mx-auto mt-6 max-w-2xl text-lg leading-8">
          Start local for free, add personal cloud features with Pro, put
          collaboration on Team, and reserve organization-wide controls for
          Enterprise.
        </p>
      </div>

      <div className="relative left-1/2 mt-8 grid w-screen max-w-[1380px] -translate-x-1/2 grid-cols-1 gap-4 px-5 text-left md:grid-cols-2 md:px-8 lg:grid-cols-4">
        {MARKETING_PLAN_TIERS.map((plan) => (
          <PricingCard key={plan.id} plan={plan} />
        ))}
      </div>

      {compareLink ? (
        <div className="mt-8">
          <Link
            to="/pricing/"
            className="text-color-muted hover:text-color text-sm underline decoration-current underline-offset-4 transition-colors"
          >
            Compare with other AI notetakers
          </Link>
        </div>
      ) : null}
    </section>
  );
}

function PricingCard({ plan }: { plan: MarketingPlanData }) {
  const visibleFeatures = plan.features.filter((feature) => feature.included);

  return (
    <article
      className={cn([
        "surface border-color-subtle flex min-h-[32rem] flex-col rounded-3xl border p-6 shadow-sm transition-all duration-200 [corner-shape:squircle]",
        plan.popular
          ? "border-color-bright ring-fg/10 shadow-lg ring-1"
          : "hover:border-color-brand hover:shadow-md",
      ])}
    >
      <div className="flex items-start">
        <h3 className="text-color font-mono text-2xl leading-none font-semibold">
          {plan.name}
        </h3>
      </div>

      <p className="text-color-secondary mt-4 min-h-[4.5rem] text-sm leading-6">
        {plan.description}
      </p>

      <div className="mt-5 min-h-[4rem]">
        <PlanPrice price={plan.price} />
      </div>

      <div className="mt-5">
        <PlanFeatureList features={visibleFeatures} dense />
      </div>

      <div className="mt-auto pt-6">
        <Link
          to={plan.id === "enterprise" ? "/enterprise/" : "/download/"}
          className={cn([
            "flex h-11 w-full items-center justify-center rounded-full font-mono text-sm font-medium transition-all hover:scale-[102%] active:scale-[98%]",
            plan.popular
              ? "bg-linear-to-t from-stone-600 to-stone-500 text-white"
              : "surface-subtle text-color hover:bg-page",
          ])}
        >
          {getPlanActionLabel(plan)}
        </Link>
      </div>
    </article>
  );
}

function PlanPrice({ price }: { price: MarketingPlanPrice }) {
  if (price.kind === "custom") {
    return (
      <div className="flex flex-col gap-1">
        <span className="text-color font-mono text-4xl leading-none font-semibold">
          Custom
        </span>
        <span className="text-color-muted text-sm">Founder-led rollout</span>
      </div>
    );
  }

  if (price.kind === "free") {
    return (
      <div className="flex items-baseline gap-2">
        <span className="text-color font-mono text-4xl leading-none font-semibold">
          $0
        </span>
        <span className="text-color-muted text-sm">/month</span>
      </div>
    );
  }

  const unit = price.billingUnit === "person" ? "/person" : "";

  return (
    <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
      <span className="text-color font-mono text-4xl leading-none font-semibold">
        ${price.monthly}
      </span>
      <span className="text-color-muted text-sm">{unit}/month</span>
      {price.yearly != null ? (
        <span className="text-color-muted text-sm">
          or ${price.yearly}
          {unit}/year
        </span>
      ) : null}
    </div>
  );
}

function getPlanActionLabel(plan: MarketingPlanData) {
  switch (plan.id) {
    case "free":
      return "Download for free";
    case "pro":
      return "Start your 3-week Pro trial";
    case "team":
      return "Create a Team workspace";
    case "enterprise":
      return "Talk to sales";
  }
}
