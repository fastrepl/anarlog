export const ANARLOG_SITE_URL = "https://anarlog.so";
export const DEFAULT_OG_IMAGE_URL = `${ANARLOG_SITE_URL}/og.jpg`;
export const ROOT_TITLE = "Anarlog - Meeting Notes You Own";
export const ROOT_DESCRIPTION =
  "Private, bot-free meeting notes that stay under your control. Anarlog stores notes as files you own and works fully offline with on-device models or your own keys.";
export const ROOT_KEYWORDS =
  "private meeting notes, bot-free AI notes, local transcription, AI meeting notes, AI notetaker, meeting transcription, meeting summaries, BYOK AI, open source note taking, local AI";

export function getBlogOgImageUrl(slug: string) {
  return `${ANARLOG_SITE_URL}/api/og/blog/${encodeURIComponent(slug)}`;
}

export function getPublicSharedNoteOgImageUrl(publicSlug: string) {
  return `${ANARLOG_SITE_URL}/api/og/share/public/${encodeURIComponent(publicSlug)}`;
}

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
    name: "Anarlog",
    url: ANARLOG_SITE_URL,
    logo: `${ANARLOG_SITE_URL}/logo.svg`,
  };
}

export function getSoftwareApplicationJsonLd({
  url = ANARLOG_SITE_URL,
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
    name: "Anarlog",
    url,
    description,
    applicationCategory: "ProductivityApplication",
    operatingSystem: "macOS",
    downloadUrl: ANARLOG_SITE_URL,
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
