export const CHAR_SITE_URL = "https://char.com";
export const DEFAULT_OG_IMAGE_URL =
  "https://auth.hyprnote.com/storage/v1/object/public/blog/brand-assets/OG.png";
export const ROOT_TITLE = "Char - Meeting Notes You Own";
export const ROOT_DESCRIPTION =
  "Private, bot-free meeting notes that stay under your control. Char stores notes as files you own and lets you use local models, your own keys, or managed cloud AI.";
export const ROOT_KEYWORDS =
  "private meeting notes, bot-free AI notes, local transcription, AI meeting notes, AI notetaker, meeting transcription, meeting summaries, BYOK AI, open source note taking, local AI";

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
