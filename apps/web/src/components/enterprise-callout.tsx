import { Link } from "@tanstack/react-router";

export function EnterpriseCallout() {
  return (
    <div className="flex flex-col items-center gap-4 rounded-3xl border border-[#eadfce] bg-[#fffaf0] p-6 text-left [corner-shape:squircle] md:flex-row md:justify-between">
      <div>
        <p className="text-sm font-semibold text-[#181613]">
          Rolling Anarlog out to a team?
        </p>
        <p className="mt-1 text-sm leading-6 text-[#4f4940]">
          Workspace admin, SSO, and a self-hosted server for regulated
          environments — shaped with early partners.
        </p>
      </div>
      <Link
        to="/enterprise/"
        className="inline-flex h-11 shrink-0 items-center justify-center rounded-full bg-[#181613] px-6 text-sm font-medium text-white transition-all hover:scale-[102%] hover:bg-[#4f4940] active:scale-[98%]"
      >
        Talk to sales
      </Link>
    </div>
  );
}
