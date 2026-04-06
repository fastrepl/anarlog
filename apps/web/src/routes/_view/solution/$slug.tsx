import { createFileRoute, notFound } from "@tanstack/react-router";
import { allSolutions } from "content-collections";

import { CTASection } from "@/components/cta-section";
import { SolutionFeatures } from "@/components/sections/solution-features";
import { SolutionHero } from "@/components/sections/solution-hero";
import { SolutionUseCases } from "@/components/sections/solution-use-cases";

export const Route = createFileRoute("/_view/solution/$slug")({
  component: Component,
  loader: async ({ params }) => {
    const doc = allSolutions.find((doc) => doc.slug === params.slug);
    if (!doc) {
      throw notFound();
    }
    return { doc };
  },
  head: ({ loaderData }) => {
    if (!loaderData?.doc) {
      return { meta: [] };
    }

    const { doc } = loaderData;

    return {
      meta: [
        { title: doc.metaTitle },
        { name: "description", content: doc.metaDescription },
        { name: "robots", content: "noindex, nofollow" },
        { property: "og:title", content: doc.metaTitle },
        { property: "og:description", content: doc.metaDescription },
        { property: "og:type", content: "website" },
        {
          property: "og:url",
          content: `https://char.com/solution/${doc.slug}`,
        },
      ],
    };
  },
});

function Component() {
  const { doc } = Route.useLoaderData();

  return (
    <div className="min-h-screen overflow-x-hidden">
      <div className="mx-auto">
        <SolutionHero
          icon={doc.icon}
          badgeText={doc.hero.badgeText}
          title={doc.hero.title}
          description={doc.hero.description}
          primaryCTA={doc.hero.primaryCTA}
          secondaryCTA={doc.hero.secondaryCTA}
        />
        <SolutionFeatures
          title={doc.features.title}
          description={doc.features.description}
          items={doc.features.items}
        />
        <SolutionUseCases
          title={doc.useCases.title}
          description={doc.useCases.description}
          items={doc.useCases.items}
        />
        <CTASection title={doc.cta.title} description={doc.cta.description} />
      </div>
    </div>
  );
}
