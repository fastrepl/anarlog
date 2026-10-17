import { useQuery } from "@tanstack/react-query";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";
import { commands as fsCommands } from "@anlg/plugin-fs2";
import { commands as settingsCommands } from "@anlg/plugin-settings";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { executeTransaction, liveQueryClient } from "~/db";
import { withCloudsyncActivity } from "~/db/cloudsync-activity";
import { getEnhancerService } from "~/services/enhancer";
import { catalogLocalSessionAudio } from "~/session/attachments";
import { useSession } from "~/session/queries";
import { useAiProvidersState } from "~/settings/providers";
import { useSettingsReady } from "~/settings/queries";
import { useRunBatch } from "~/stt/useRunBatch";

const workerId = crypto.randomUUID();
type ImportJob = { id: string; meeting_id: string; file_path: string };

async function heartbeat() {
  const vault = await settingsCommands.vaultBase();
  if (vault.status === "error") throw new Error(vault.error);
  await executeTransaction([
    {
      sql: `INSERT INTO local_recording_import_worker (id, vault_path, updated_at) VALUES (?, ?, unixepoch())
        ON CONFLICT(id) DO UPDATE SET updated_at = excluded.updated_at, vault_path = excluded.vault_path`,
      params: [workerId, vault.data],
    },
    {
      sql: `UPDATE local_recording_import_jobs SET processing_status = 'failed',
        error = 'Desktop processing was interrupted. The meeting and any completed transcript were preserved.',
        updated_at = unixepoch()
        WHERE processing_status IN ('importing', 'transcribing', 'summarizing') AND updated_at < unixepoch() - 120`,
      params: [],
    },
    {
      sql: `UPDATE local_recording_import_jobs SET processing_status = 'failed', error = 'Meeting was deleted.', updated_at = unixepoch()
        WHERE processing_status = 'queued' AND NOT EXISTS (SELECT 1 FROM sessions WHERE sessions.id = meeting_id AND deleted_at IS NULL)`,
      params: [],
    },
    {
      sql: `UPDATE local_recording_import_jobs SET processing_status = 'failed', error = 'Recording import is disabled by managed policy.', updated_at = unixepoch()
        WHERE processing_status = 'queued' AND EXISTS (SELECT 1 FROM managed_app_settings WHERE id = 'intelligence_disabled' AND value_json = 'true')`,
      params: [],
    },
  ]);
  const completed = await liveQueryClient.execute<ImportJob>(
    `SELECT id, meeting_id, file_path FROM local_recording_import_jobs WHERE processing_status IN ('done', 'failed') AND file_path != ''`,
    [],
  );
  for (const job of completed) {
    const removed = await fsCommands.remove(job.file_path);
    if (removed.status === "ok")
      await executeTransaction([
        {
          sql: `UPDATE local_recording_import_jobs SET file_path = '' WHERE id = ?`,
          params: [job.id],
        },
      ]);
  }
}

export function RecordingImportWorker() {
  useMountEffect(() => {
    const tick = () => void heartbeat().catch(console.error);
    tick();
    const timer = setInterval(tick, 5000);
    return () => {
      clearInterval(timer);
    };
  });
  const { data: jobs = [] } = useQuery({
    queryKey: ["local-recording-import-jobs"],
    queryFn: () =>
      liveQueryClient.execute<ImportJob>(
        `SELECT id, meeting_id, file_path FROM local_recording_import_jobs
       WHERE processing_status = 'queued' OR (worker_id = ? AND processing_status IN ('importing', 'transcribing', 'summarizing'))
       ORDER BY CASE WHEN processing_status = 'queued' THEN 1 ELSE 0 END, updated_at, id LIMIT 1`,
        [workerId],
      ),
    refetchInterval: 2000,
    refetchIntervalInBackground: true,
  });
  return (
    <>
      {jobs.map((job) => (
        <ReadyImport key={job.id} job={job} />
      ))}
    </>
  );
}

function ReadyImport({ job }: { job: ImportJob }) {
  const session = useSession(job.meeting_id);
  const settingsReady = useSettingsReady();
  const stt = useAiProvidersState("stt");
  const llm = useAiProvidersState("llm");
  const billing = useBillingAccess();
  const auth = useAuth();
  // Signed-out users have no billing claims to load; local providers still run.
  const billingSettled = billing.isReady || auth.session === null;
  return session &&
    settingsReady &&
    stt.isReady &&
    llm.isReady &&
    billingSettled ? (
    <RunImport job={job} />
  ) : null;
}

const runningImports = new Set<string>();

function RunImport({ job }: { job: ImportJob }) {
  const runBatch = useRunBatch(job.meeting_id);
  useMountEffect(() => {
    // Defer acquisition until after React's mount/cleanup probe. Once started,
    // processing outlives this component so navigation does not fail the job.
    const start = setTimeout(() => {
      if (runningImports.has(job.id)) return;
      runningImports.add(job.id);
      void processImport(job, runBatch)
        .catch(console.error)
        .finally(() => runningImports.delete(job.id));
    }, 0);
    return () => {
      clearTimeout(start);
    };
  });
  return null;
}

async function processImport(
  job: ImportJob,
  runBatch: ReturnType<typeof useRunBatch>,
) {
  const update = async (
    status: string,
    summary = "pending",
    error: string | null = null,
  ) => {
    await executeTransaction([
      {
        sql: `UPDATE local_recording_import_jobs SET processing_status = ?, summary_status = ?, error = ?, updated_at = unixepoch()
        WHERE id = ? AND worker_id = ? AND processing_status NOT IN ('done', 'failed')`,
        params: [status, summary, error, job.id, workerId],
        ...(status !== "failed" ? { expectedRowsAffected: 1 } : {}),
      },
    ]);
  };
  const claimed = await liveQueryClient.execute<{ id: string }>(
    `UPDATE local_recording_import_jobs SET worker_id = ?, processing_status = 'importing', updated_at = unixepoch()
     WHERE id = ? AND processing_status = 'queued'
     AND NOT EXISTS (SELECT 1 FROM managed_app_settings WHERE id = 'intelligence_disabled' AND value_json = 'true') RETURNING id`,
    [workerId, job.id],
  );
  if (!claimed.length) return;
  const timer = setInterval(() => {
    void executeTransaction([
      {
        sql: `UPDATE local_recording_import_jobs SET updated_at = unixepoch() WHERE id = ? AND worker_id = ? AND processing_status NOT IN ('done', 'failed')`,
        params: [job.id, workerId],
      },
    ]).catch(console.error);
  }, 10000);
  let phase = "importing";
  try {
    await withCloudsyncActivity(
      "transcription",
      `recording-import:${job.id}`,
      async () => {
        const imported = await fsSyncCommands.audioImport(
          job.meeting_id,
          job.file_path,
        );
        if (imported.status === "error") throw new Error(imported.error);
        await catalogLocalSessionAudio(job.meeting_id);
        phase = "transcribing";
        await update(phase);
        await runBatch(imported.data, {
          promotion: { scope: "whole_session" },
        });
        phase = "summarizing";
        await update(phase);
        const enhancer = getEnhancerService();
        if (!enhancer)
          throw new Error(
            "Intelligence service is not ready. Transcript was preserved.",
          );
        const summary = await enhancer.enhanceAndWait(job.meeting_id);
        await update("done", summary);
      },
    );
  } catch {
    // Provider errors may contain credentials; keep externally readable job errors bounded and generic.
    await update(
      "failed",
      phase === "summarizing" ? "failed" : "pending",
      `${phase} failed or was interrupted. Check the desktop app; completed meeting data was preserved.`,
    );
  } finally {
    clearInterval(timer);
  }
}
