function ordinalSuffix(day: number): string {
  if (day >= 11 && day <= 13) {
    return "th";
  }

  switch (day % 10) {
    case 1:
      return "st";
    case 2:
      return "nd";
    case 3:
      return "rd";
    default:
      return "th";
  }
}

function formatDateHeader(date: string): string {
  const value = new Date(`${date}T00:00:00`);
  const month = value.toLocaleDateString("en-US", { month: "long" });
  const day = value.getDate();
  return `${month} ${day}${ordinalSuffix(day)}`;
}

function getTodayString(): string {
  const value = new Date();
  return `${value.getFullYear()}-${String(value.getMonth() + 1).padStart(2, "0")}-${String(value.getDate()).padStart(2, "0")}`;
}

export function DateHeaderView({
  date,
  muted,
  inline,
  onOpenDailySummary,
}: {
  date: string;
  muted?: boolean;
  inline?: boolean;
  onOpenDailySummary?: (date: string) => void;
}) {
  const isToday = date === getTodayString();
  const labelClassName = muted
    ? "text-lg font-medium text-neutral-400"
    : "text-xl font-semibold text-neutral-900";

  const label = formatDateHeader(date);
  const content = (
    <>
      {onOpenDailySummary ? (
        <button
          type="button"
          onClick={() => onOpenDailySummary(date)}
          className={`${labelClassName} transition-colors hover:text-neutral-600`}
        >
          {label}
        </button>
      ) : (
        <span className={labelClassName}>{label}</span>
      )}
      {isToday ? (
        <span className="rounded-full bg-neutral-900 px-2 py-0.5 text-xs font-medium text-white">
          Today
        </span>
      ) : null}
    </>
  );

  if (inline) {
    return <div className="flex items-center gap-3">{content}</div>;
  }

  return (
    <div className="flex items-center gap-3 px-6 pt-6 pb-3">{content}</div>
  );
}
