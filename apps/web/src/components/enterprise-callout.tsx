import { Link } from "@tanstack/react-router";

const heading = "Rolling Anarlog out to a team?";
const body =
  "Forward the enterprise page to IT: encryption, retention, training, and subprocessors, then a founder-led pilot.";
const ctaClassName =
  "inline-flex h-11 shrink-0 items-center justify-center rounded-full bg-[#181613] px-6 text-sm font-medium text-white transition-all hover:scale-[102%] hover:bg-[#4f4940] active:scale-[98%]";

// `centered` drops the card treatment for pages that have no other cards to
// sit alongside.
export function EnterpriseCallout({
  centered = false,
}: {
  centered?: boolean;
}) {
  if (centered) {
    return (
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#181613]">
          {heading}
        </h2>
        <p className="mx-auto mt-5 max-w-lg text-base leading-7 text-[#4f4940]">
          {body}
        </p>
        <div className="mt-8">
          <Link to="/enterprise/" className={ctaClassName}>
            Talk to sales
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-4 rounded-3xl border border-[#eadfce] bg-[#fffaf0] p-6 text-left [corner-shape:squircle] md:flex-row md:justify-between">
      <div>
        <p className="text-sm font-semibold text-[#181613]">{heading}</p>
        <p className="mt-1 text-sm leading-6 text-[#4f4940]">{body}</p>
      </div>
      <Link to="/enterprise/" className={ctaClassName}>
        Talk to sales
      </Link>
    </div>
  );
}
