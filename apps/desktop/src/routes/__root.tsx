import { createRootRouteWithContext } from "@tanstack/react-router";
import { lazy, Suspense } from "react";

import { BrandLoadingView } from "~/shared/brand-loading-view";
import type { Context } from "~/types";

const MainAppLayout = lazy(() => import("~/shared/main-app-layout"));

export const Route = createRootRouteWithContext<Partial<Context>>()({
  component: Component,
});

function Component() {
  return (
    <Suspense
      fallback={
        <BrandLoadingView detail="Loading your workspace. This should only take a moment." />
      }
    >
      <MainAppLayout />
    </Suspense>
  );
}
