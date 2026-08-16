import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

import { fetchUser } from "@/functions/auth";
import { useMountEffect } from "@/hooks/useMountEffect";
import { setErrorReportingUser } from "@/lib/error-reporting";
import { isDesktopIntegrationHandoff } from "@/lib/integration-handoff";

export const Route = createFileRoute("/_view/app")({
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
  beforeLoad: async ({ location }) => {
    if (
      isDesktopIntegrationHandoff({
        pathname: location.pathname,
        search: location.search as Record<string, unknown>,
      })
    ) {
      return { user: null };
    }

    const user = await fetchUser();
    if (!user) {
      const searchStr =
        Object.keys(location.search).length > 0
          ? `?${new URLSearchParams(location.search as Record<string, string>).toString()}`
          : "";
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          redirect: location.pathname + searchStr,
        },
      });
    }
    return { user };
  },
  component: AppRoute,
});

function AppRoute() {
  const { user } = Route.useRouteContext();

  if (!user) {
    return <Outlet />;
  }

  return (
    <ErrorReportingIdentity key={user.id} userId={user.id}>
      <Outlet />
    </ErrorReportingIdentity>
  );
}

function ErrorReportingIdentity({
  children,
  userId,
}: {
  children: React.ReactNode;
  userId: string;
}) {
  useMountEffect(() => {
    setErrorReportingUser(userId);
    return () => setErrorReportingUser(null);
  });

  return children;
}
