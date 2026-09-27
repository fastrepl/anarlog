import { useLingui } from "@lingui/react";

export function MessageTimestamp({
  createdAt,
  showDate = false,
}: {
  createdAt?: number;
  showDate?: boolean;
}) {
  const { i18n } = useLingui();
  if (createdAt === undefined || !Number.isFinite(createdAt)) {
    return null;
  }
  const date = new Date(createdAt);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  const locale = i18n.locale || undefined;
  const time = date.toLocaleTimeString(locale, {
    hour: "numeric",
    minute: "2-digit",
  });
  let label = time;
  if (showDate) {
    const today = new Date();
    const yesterday = new Date(today);
    yesterday.setDate(today.getDate() - 1);
    const relativeDay =
      date.toDateString() === today.toDateString()
        ? 0
        : date.toDateString() === yesterday.toDateString()
          ? -1
          : undefined;
    const day =
      relativeDay === undefined
        ? date.toLocaleDateString(locale, {
            month: "short",
            day: "numeric",
            ...(date.getFullYear() !== today.getFullYear()
              ? { year: "numeric" as const }
              : {}),
          })
        : new Intl.RelativeTimeFormat(locale, { numeric: "auto" }).format(
            relativeDay,
            "day",
          );
    label = `${day} ${time}`;
  }

  return (
    <time
      dateTime={date.toISOString()}
      title={date.toLocaleString(locale)}
      className="text-muted-foreground inline-block text-xs first-letter:uppercase"
    >
      {label}
    </time>
  );
}
