export function formatBlogDate(date: string, month: "long" | "short") {
  const parsed = new Date(date);

  if (Number.isNaN(parsed.getTime())) {
    return date;
  }

  return new Intl.DateTimeFormat("en-US", {
    month,
    day: "numeric",
    year: "numeric",
    timeZone: "UTC",
  }).format(parsed);
}
