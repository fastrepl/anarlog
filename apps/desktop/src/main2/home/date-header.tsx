function ordinalSuffix(day: number): string {
  if (day >= 11 && day <= 13) return "th";
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

function formatDateHeader(dateStr: string): string {
  const date = new Date(dateStr + "T00:00:00");
  const month = date.toLocaleDateString("en-US", { month: "long" });
  const day = date.getDate();
  return `${month} ${day}${ordinalSuffix(day)}`;
}

export function DateHeader({ date, muted }: { date: string; muted?: boolean }) {
  return (
    <div className="flex items-center gap-3 px-6 pt-6 pb-3">
      <h2
        className={
          muted
            ? "text-lg font-medium text-neutral-400"
            : "text-xl font-semibold text-neutral-900"
        }
      >
        {formatDateHeader(date)}
      </h2>
    </div>
  );
}
