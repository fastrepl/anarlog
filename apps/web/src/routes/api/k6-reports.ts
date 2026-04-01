import { createFileRoute } from "@tanstack/react-router";
import { json } from "@tanstack/react-start";
import JSZip from "jszip";

import { env } from "../../env";

const REPO = "char";
const OWNER = "fastrepl";

export const Route = createFileRoute("/api/k6-reports")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const url = new URL(request.url);
        const artifactId = url.searchParams.get("artifact_id");

        if (artifactId) {
          return fetchReportContent(artifactId);
        }

        return fetchReportsList();
      },
    },
  },
});

async function fetchReportsList() {
  const res = await fetch(
    `https://api.github.com/repos/${OWNER}/${REPO}/actions/artifacts?per_page=30&name=k6-report`,
    {
      headers: {
        Authorization: `Bearer ${env.GITHUB_TOKEN}`,
        Accept: "application/vnd.github+json",
      },
    },
  );

  if (!res.ok) {
    return json({ error: "Failed to fetch artifacts" }, { status: res.status });
  }

  const data = await res.json();

  const reports = data.artifacts
    .filter((a: { expired: boolean }) => !a.expired)
    .map(
      (a: {
        id: number;
        name: string;
        created_at: string;
        workflow_run: { id: number };
      }) => ({
        id: a.id,
        name: a.name,
        created_at: a.created_at,
        run_id: a.workflow_run?.id,
      }),
    );

  return json({ reports });
}

async function fetchReportContent(artifactId: string) {
  const res = await fetch(
    `https://api.github.com/repos/${OWNER}/${REPO}/actions/artifacts/${artifactId}/zip`,
    {
      headers: {
        Authorization: `Bearer ${env.GITHUB_TOKEN}`,
        Accept: "application/vnd.github+json",
      },
    },
  );

  if (!res.ok) {
    return json({ error: "Failed to fetch artifact" }, { status: res.status });
  }

  const buffer = await res.arrayBuffer();
  const zip = await JSZip.loadAsync(buffer);

  const jsonFile = Object.keys(zip.files).find((name) =>
    name.endsWith(".json"),
  );
  if (!jsonFile) {
    return json({ error: "No JSON file in artifact" }, { status: 404 });
  }

  const content = await zip.files[jsonFile].async("string");
  const report = JSON.parse(content);

  return json({ report });
}
