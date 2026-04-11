export function withCharUtm(
  url: string,
  {
    source,
    medium,
    campaign = "organic",
  }: {
    source: string;
    medium: string;
    campaign?: string;
  },
) {
  const parsed = new URL(url);

  parsed.searchParams.set("utm_source", source);
  parsed.searchParams.set("utm_medium", medium);
  parsed.searchParams.set("utm_campaign", campaign);

  return parsed.toString();
}
