import { useAnalytics } from "@/hooks/use-posthog";
import {
  BOOK_CALL_URL,
  ENTERPRISE_EVENTS,
  type EnterpriseCtaLocation,
  type EnterpriseSurface,
} from "@/lib/enterprise";

export function BookFounderCall({
  location,
  page,
}: {
  location: EnterpriseCtaLocation;
  page: EnterpriseSurface;
}) {
  const { track } = useAnalytics();

  return (
    <a
      href={BOOK_CALL_URL}
      target="_blank"
      rel="noopener noreferrer"
      onClick={() =>
        track(ENTERPRISE_EVENTS.ctaClicked, {
          cta: "book_call",
          location,
          page,
        })
      }
      className="inline-flex h-11 items-center justify-center rounded-full bg-[#181613] px-6 text-sm font-medium text-white transition-all hover:scale-[102%] hover:bg-[#4f4940] active:scale-[98%]"
    >
      Book a call with the founder
    </a>
  );
}
