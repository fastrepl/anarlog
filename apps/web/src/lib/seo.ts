export const CHAR_SITE_URL = "https://char.com";
export const DEFAULT_OG_IMAGE_URL = `${CHAR_SITE_URL}/api/assets/blog/brand-assets/OG.png`;
export const ROOT_TITLE = "Char - AI Daily Notes That Remember and Act";
export const ROOT_DESCRIPTION =
  "Char records your meetings without bots, pulls action items from your emails, and builds a daily note with everything you need. Review it, hand off the rest to AI agents like Claude or Cursor. Local-first, open source, your data stays yours.";
export const ROOT_KEYWORDS =
  "AI daily notes, daily note app, bot-free meeting notes, local transcription, AI notetaker, meeting summaries, screen recording AI, task delegation AI, BYOK AI, open source notes, local-first AI";

type StructuredDataNode = Record<string, unknown>;

export function getStructuredDataGraph(nodes: StructuredDataNode[]) {
  return {
    "@context": "https://schema.org",
    "@graph": nodes,
  };
}

export function getOrganizationJsonLd() {
  return {
    "@type": "Organization",
    name: "Char",
    url: CHAR_SITE_URL,
    logo: `${CHAR_SITE_URL}/favicon.svg`,
  };
}

export function getSoftwareApplicationJsonLd({
  url = CHAR_SITE_URL,
  description,
  featureList,
  aggregateOffer,
}: {
  url?: string;
  description: string;
  featureList?: string[];
  aggregateOffer?: {
    lowPrice: number;
    highPrice: number;
    offerCount: number;
  };
}) {
  return {
    "@type": "SoftwareApplication",
    name: "Char",
    url,
    description,
    applicationCategory: "ProductivityApplication",
    operatingSystem: "macOS",
    downloadUrl: `${CHAR_SITE_URL}/download`,
    publisher: getOrganizationJsonLd(),
    ...(featureList ? { featureList } : {}),
    ...(aggregateOffer
      ? {
          offers: {
            "@type": "AggregateOffer",
            url,
            priceCurrency: "USD",
            ...aggregateOffer,
          },
        }
      : {}),
  };
}

export function getBreadcrumbListJsonLd(
  items: Array<{ name: string; item: string }>,
) {
  return {
    "@type": "BreadcrumbList",
    itemListElement: items.map((item, index) => ({
      "@type": "ListItem",
      position: index + 1,
      name: item.name,
      item: item.item,
    })),
  };
}
