import type { changelogEntries } from "@/lib/changelog";
import { availabilityLabel } from "@/lib/changelog-data";

export function ChangelogAvailability({
  entry,
}: {
  entry: (typeof changelogEntries)[number];
}) {
  if (entry.stream !== "mobile") return null;
  return (
    <div className="text-brand-dark mt-3 text-xs leading-5">
      <span className="font-semibold">
        {entry.channel === "beta"
          ? "Beta"
          : entry.channel === "stable"
            ? "Stable"
            : "Draft"}
      </span>
      <ul>
        {entry.availability.map(({ platform, destination }) => (
          <li key={platform}>
            {platform === "ios" ? "iOS" : "Android"} ·{" "}
            {availabilityLabel(destination)}
          </li>
        ))}
      </ul>
    </div>
  );
}
