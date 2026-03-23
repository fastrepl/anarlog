import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/k6-reports/")({
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

interface Report {
  id: number;
  name: string;
  created_at: string;
  run_id?: number;
}

function Component() {
  const { data, isLoading, error } = useQuery({
    queryKey: ["k6-reports"],
    queryFn: async () => {
      const res = await fetch("/api/k6-reports");
      if (!res.ok) throw new Error("Failed to fetch reports");
      return res.json() as Promise<{ reports: Report[] }>;
    },
  });

  if (isLoading) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16">
        <div className="flex animate-pulse flex-col gap-4">
          <div className="h-8 w-1/3 rounded bg-neutral-200" />
          <div className="h-4 w-1/2 rounded bg-neutral-200" />
          <div className="mt-8 flex flex-col gap-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-16 rounded bg-neutral-100" />
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16">
        <h1 className="text-2xl font-bold text-red-600">Error</h1>
        <p className="mt-2 text-neutral-600">{(error as Error).message}</p>
      </div>
    );
  }

  const reports = data?.reports || [];

  return (
    <div className="mx-auto max-w-4xl px-4 py-16">
      <h1 className="text-3xl font-bold">K6 Load Test Reports</h1>
      <p className="mt-2 text-neutral-600">
        WebSocket stability test results from CI
      </p>

      {reports.length === 0 ? (
        <div className="mt-8 rounded-lg bg-neutral-50 p-8 text-center">
          <p className="text-neutral-500">No reports available</p>
        </div>
      ) : (
        <div className="mt-8 flex flex-col gap-3">
          {reports.map((report) => (
            <Link
              key={report.id}
              to="/k6-reports/$id/"
              params={{ id: String(report.id) }}
              className="block rounded-lg border border-neutral-200 bg-white p-4 transition-all hover:border-neutral-300 hover:shadow-xs"
            >
              <div className="flex items-center justify-between">
                <div>
                  <div className="font-medium">{report.name}</div>
                  <div className="text-sm text-neutral-500">
                    {new Date(report.created_at).toLocaleString()}
                  </div>
                </div>
                {report.run_id && (
                  <a
                    href={`https://github.com/fastrepl/char/actions/runs/${report.run_id}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    onClick={(e) => e.stopPropagation()}
                    className="text-sm text-blue-600 hover:underline"
                  >
                    View Run →
                  </a>
                )}
              </div>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}
