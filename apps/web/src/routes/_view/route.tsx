import { createFileRoute, Outlet } from "@tanstack/react-router";

import { brandPageNoiseBackgroundImage } from "@/lib/brand-noise";

export const Route = createFileRoute("/_view")({
  component: Component,
});

function Component() {
  return (
    <div className="bg-page relative min-h-screen overflow-hidden">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-[130vh]"
        style={{
          background:
            "linear-gradient(to bottom, var(--brand-yellow), transparent)",
        }}
      />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-x-0 top-0 h-[130vh] opacity-30"
        style={{
          backgroundImage: brandPageNoiseBackgroundImage,
          backgroundRepeat: "repeat",
          maskImage: "linear-gradient(to bottom, black, transparent)",
          WebkitMaskImage: "linear-gradient(to bottom, black, transparent)",
        }}
      />
      <div className="relative z-10">
        <Outlet />
      </div>
    </div>
  );
}
