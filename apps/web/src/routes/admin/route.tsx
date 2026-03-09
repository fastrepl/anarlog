import {
  createFileRoute,
  Link,
  Outlet,
  redirect,
} from "@tanstack/react-router";

import { fetchAdminUser } from "@/functions/admin";

export const Route = createFileRoute("/admin")({
  head: () => ({
    meta: [
      { title: "Admin - Char" },
      { name: "description", content: "Char admin dashboard." },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
  beforeLoad: async ({ location }) => {
    if (import.meta.env.DEV) {
      return { user: { email: "dev@local", isAdmin: true } };
    }

    const user = await fetchAdminUser();

    if (!user) {
      throw redirect({
        to: "/auth/",
        search: {
          flow: "web",
          provider: "github",
          redirect: location.pathname,
          rra: true,
        },
      });
    }

    if (!user.isAdmin) {
      throw redirect({
        to: "/",
      });
    }

    return { user };
  },
  component: AdminLayout,
});

function AdminLayout() {
  const { user } = Route.useRouteContext();

  return (
    <div className="flex h-screen flex-col bg-white">
      <AdminHeader user={user} />
      <main className="min-h-0 flex-1">
        <Outlet />
      </main>
    </div>
  );
}

function AdminHeader({ user }: { user: { email: string } }) {
  const firstName = user.email.split("@")[0].split(".")[0];
  const displayName = firstName.charAt(0).toUpperCase() + firstName.slice(1);

  return (
    <header className="h-16 border-b border-neutral-200 bg-white">
      <div className="flex h-full items-center justify-between px-6">
        <div className="flex items-center gap-6">
          <Link
            to="/admin/"
            className="font-serif2 text-2xl text-stone-600 italic"
          >
            Char Admin
          </Link>
          <nav className="flex items-center gap-4">
            <Link
              to="/admin/collections/"
              className="relative py-1 text-sm font-medium text-neutral-600 transition-colors hover:text-neutral-900 [&.active]:text-neutral-900 [&.active]:after:absolute [&.active]:after:bottom-0 [&.active]:after:left-1/2 [&.active]:after:h-0.5 [&.active]:after:w-7 [&.active]:after:-translate-x-1/2 [&.active]:after:rounded-full [&.active]:after:bg-neutral-900"
              activeProps={{ className: "active" }}
            >
              Articles
            </Link>
            <Link
              to="/admin/media/"
              className="relative py-1 text-sm font-medium text-neutral-600 transition-colors hover:text-neutral-900 [&.active]:text-neutral-900 [&.active]:after:absolute [&.active]:after:bottom-0 [&.active]:after:left-1/2 [&.active]:after:h-0.5 [&.active]:after:w-7 [&.active]:after:-translate-x-1/2 [&.active]:after:rounded-full [&.active]:after:bg-neutral-900"
              activeProps={{ className: "active" }}
            >
              Media
            </Link>
            <div className="h-4 w-px bg-neutral-300" />
            <Link
              to="/admin/crm/"
              className="relative py-1 text-sm font-medium text-neutral-600 transition-colors hover:text-neutral-900 [&.active]:text-neutral-900 [&.active]:after:absolute [&.active]:after:bottom-0 [&.active]:after:left-1/2 [&.active]:after:h-0.5 [&.active]:after:w-7 [&.active]:after:-translate-x-1/2 [&.active]:after:rounded-full [&.active]:after:bg-neutral-900"
              activeProps={{ className: "active" }}
            >
              CRM
            </Link>
            <Link
              to="/admin/lead-finder/"
              className="relative py-1 text-sm font-medium text-neutral-600 transition-colors hover:text-neutral-900 [&.active]:text-neutral-900 [&.active]:after:absolute [&.active]:after:bottom-0 [&.active]:after:left-1/2 [&.active]:after:h-0.5 [&.active]:after:w-7 [&.active]:after:-translate-x-1/2 [&.active]:after:rounded-full [&.active]:after:bg-neutral-900"
              activeProps={{ className: "active" }}
            >
              Lead Finder
            </Link>
            <div className="h-4 w-px bg-neutral-300" />
            <Link
              to="/admin/kanban/"
              className="relative py-1 text-sm font-medium text-neutral-600 transition-colors hover:text-neutral-900 [&.active]:text-neutral-900 [&.active]:after:absolute [&.active]:after:bottom-0 [&.active]:after:left-1/2 [&.active]:after:h-0.5 [&.active]:after:w-7 [&.active]:after:-translate-x-1/2 [&.active]:after:rounded-full [&.active]:after:bg-neutral-900"
              activeProps={{ className: "active" }}
            >
              Kanban
            </Link>
          </nav>
        </div>

        <div className="flex items-center gap-6">
          <span className="text-sm text-neutral-600">
            Welcome {displayName}!
          </span>
          <Link
            to="/"
            className="flex h-8 items-center rounded-full border border-red-200 bg-linear-to-b from-white to-red-50 px-4 text-sm text-red-600 shadow-xs transition-all hover:scale-[102%] hover:shadow-md active:scale-[98%]"
          >
            Sign out
          </Link>
        </div>
      </div>
    </header>
  );
}
