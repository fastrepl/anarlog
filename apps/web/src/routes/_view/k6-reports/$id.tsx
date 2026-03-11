import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";

export const Route = createFileRoute("/_view/k6-reports/$id")({
  component: Component,
  head: () => ({
    meta: [{ name: "robots", content: "noindex, nofollow" }],
  }),
});

interface K6Report {
  timestamp: string;
  duration_ms: number;
  vus_max: number;
  github?: {
    run_id: string;
    repository: string;
    url: string;
  };
  client: {
    connections: {
      total: number;
      success_rate: number;
      errors: number;
      reconnects: number;
    };
    transcripts: {
      received: number;
      first_latency_avg_ms: number;
      first_latency_p95_ms: number;
    };
    connection_duration: {
      avg_ms: number;
      p95_ms: number;
    };
    checks_passed_rate: number;
  };
  server?: {
    cpu_usage?: { avg: number; max: number; min: number };
    memory_used_bytes?: { avg: number; max: number; min: number };
    memory_total_bytes?: { avg: number; max: number; min: number };
    concurrency?: { avg: number; max: number; min: number };
    net_recv_bytes?: { avg: number };
    net_sent_bytes?: { avg: number };
    error?: string;
  };
  thresholds: Record<string, { ok: boolean }>;
}

function Component() {
  const { id } = Route.useParams();

  const { data, isLoading, error } = useQuery({
    queryKey: ["k6-report", id],
    queryFn: async () => {
      const res = await fetch(`/api/k6-reports?artifact_id=${id}`);
      if (!res.ok) throw new Error("Failed to fetch report");
      return res.json() as Promise<{ report: K6Report }>;
    },
  });

  if (isLoading) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16">
        <div className="flex animate-pulse flex-col gap-4">
          <div className="h-8 w-1/3 rounded bg-neutral-200" />
          <div className="mt-8 h-64 rounded bg-neutral-100" />
        </div>
      </div>
    );
  }

  if (error || !data?.report) {
    return (
      <div className="mx-auto max-w-4xl px-4 py-16">
        <h1 className="text-2xl font-bold text-red-600">Error</h1>
        <p className="mt-2 text-neutral-600">
          {(error as Error)?.message || "Report not found"}
        </p>
        <Link
          to="/k6-reports/"
          className="mt-4 inline-block text-blue-600 hover:underline"
        >
          ← Back to reports
        </Link>
      </div>
    );
  }

  const report = data.report;

  return (
    <div className="mx-auto max-w-4xl px-4 py-16">
      <Link to="/k6-reports/" className="text-sm text-blue-600 hover:underline">
        ← Back to reports
      </Link>

      <div className="mt-4 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold">Load Test Report</h1>
          <p className="mt-1 text-neutral-500">
            {new Date(report.timestamp).toLocaleString()}
          </p>
        </div>
        {report.github && (
          <a
            href={report.github.url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-blue-600 hover:underline"
          >
            View GitHub Run →
          </a>
        )}
      </div>

      <div className="mt-8 grid grid-cols-3 gap-4">
        <MetricCard
          label="Duration"
          value={formatDuration(report.duration_ms)}
        />
        <MetricCard label="Virtual Users" value={String(report.vus_max)} />
        <MetricCard
          label="Success Rate"
          value={`${(report.client.connections.success_rate * 100).toFixed(1)}%`}
          status={
            report.client.connections.success_rate >= 0.95 ? "good" : "bad"
          }
        />
      </div>

      <Section title="Client Metrics" className="mt-8">
        <div className="grid grid-cols-2 gap-6">
          <div>
            <h4 className="mb-2 font-medium text-neutral-700">Connections</h4>
            <dl className="flex flex-col gap-1 text-sm">
              <Row label="Total" value={report.client.connections.total} />
              <Row
                label="Errors"
                value={report.client.connections.errors}
                status={report.client.connections.errors === 0 ? "good" : "bad"}
              />
              <Row
                label="Reconnects"
                value={report.client.connections.reconnects}
              />
            </dl>
          </div>
          <div>
            <h4 className="mb-2 font-medium text-neutral-700">Transcripts</h4>
            <dl className="flex flex-col gap-1 text-sm">
              <Row
                label="Received"
                value={report.client.transcripts.received}
              />
              <Row
                label="First Latency (avg)"
                value={`${report.client.transcripts.first_latency_avg_ms.toFixed(0)}ms`}
              />
              <Row
                label="First Latency (p95)"
                value={`${report.client.transcripts.first_latency_p95_ms.toFixed(0)}ms`}
              />
            </dl>
          </div>
        </div>
        <div className="mt-4">
          <h4 className="mb-2 font-medium text-neutral-700">
            Connection Duration
          </h4>
          <dl className="flex flex-col gap-1 text-sm">
            <Row
              label="Average"
              value={formatDuration(report.client.connection_duration.avg_ms)}
            />
            <Row
              label="P95"
              value={formatDuration(report.client.connection_duration.p95_ms)}
            />
          </dl>
        </div>
      </Section>

      {report.server && !report.server.error && (
        <Section title="Server Metrics (Fly.io)" className="mt-6">
          <div className="grid grid-cols-2 gap-6">
            <div>
              <h4 className="mb-2 font-medium text-neutral-700">Resources</h4>
              <dl className="flex flex-col gap-1 text-sm">
                {report.server.cpu_usage && (
                  <Row
                    label="CPU Usage"
                    value={`avg ${(report.server.cpu_usage.avg * 100).toFixed(1)}%, max ${(report.server.cpu_usage.max * 100).toFixed(1)}%`}
                  />
                )}
                {report.server.memory_used_bytes &&
                  report.server.memory_total_bytes && (
                    <Row
                      label="Memory"
                      value={`${formatBytes(report.server.memory_used_bytes.avg)} / ${formatBytes(report.server.memory_total_bytes.avg)}`}
                    />
                  )}
                {report.server.concurrency && (
                  <Row
                    label="Concurrency"
                    value={`avg ${report.server.concurrency.avg.toFixed(1)}, max ${report.server.concurrency.max.toFixed(0)}`}
                  />
                )}
              </dl>
            </div>
            <div>
              <h4 className="mb-2 font-medium text-neutral-700">Network</h4>
              <dl className="flex flex-col gap-1 text-sm">
                {report.server.net_recv_bytes && (
                  <Row
                    label="Received"
                    value={formatBytes(report.server.net_recv_bytes.avg)}
                  />
                )}
                {report.server.net_sent_bytes && (
                  <Row
                    label="Sent"
                    value={formatBytes(report.server.net_sent_bytes.avg)}
                  />
                )}
              </dl>
            </div>
          </div>
        </Section>
      )}

      <Section title="Thresholds" className="mt-6">
        <div className="flex flex-wrap gap-2">
          {Object.entries(report.thresholds).map(([key, { ok }]) => (
            <span
              key={key}
              className={[
                "px-2 py-1 rounded text-sm",
                ok ? "bg-green-100 text-green-800" : "bg-red-100 text-red-800",
              ].join(" ")}
            >
              {key}: {ok ? "✓" : "✗"}
            </span>
          ))}
        </div>
      </Section>
    </div>
  );
}

function Section({
  title,
  children,
  className,
}: {
  title: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section
      className={[
        "bg-white border border-neutral-200 rounded-lg p-6",
        className,
      ].join(" ")}
    >
      <h3 className="mb-4 text-lg font-semibold">{title}</h3>
      {children}
    </section>
  );
}

function MetricCard({
  label,
  value,
  status,
}: {
  label: string;
  value: string;
  status?: "good" | "bad";
}) {
  return (
    <div className="rounded-lg border border-neutral-200 bg-white p-4">
      <div className="text-sm text-neutral-500">{label}</div>
      <div
        className={[
          "text-2xl font-bold mt-1",
          status === "good"
            ? "text-green-600"
            : status === "bad"
              ? "text-red-600"
              : "",
        ].join(" ")}
      >
        {value}
      </div>
    </div>
  );
}

function Row({
  label,
  value,
  status,
}: {
  label: string;
  value: string | number;
  status?: "good" | "bad";
}) {
  return (
    <div className="flex justify-between">
      <dt className="text-neutral-500">{label}</dt>
      <dd
        className={[
          "font-medium",
          status === "good"
            ? "text-green-600"
            : status === "bad"
              ? "text-red-600"
              : "",
        ].join(" ")}
      >
        {value}
      </dd>
    </div>
  );
}

function formatDuration(ms: number): string {
  if (!ms) return "0s";
  if (ms < 1000) return `${ms.toFixed(0)}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  if (ms < 3600000) return `${(ms / 60000).toFixed(1)}m`;
  return `${(ms / 3600000).toFixed(2)}h`;
}

function formatBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  while (bytes >= 1024 && i < units.length - 1) {
    bytes /= 1024;
    i++;
  }
  return `${bytes.toFixed(1)} ${units[i]}`;
}
