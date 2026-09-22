import { Trans } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";

import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";

import { IntegrationGate, useAuthedApiClient } from "./starter-config";

import { pickDriveFolder } from "~/automations/drive-picker";
import { retryDriveExport } from "~/automations/engine";
import type { AutomationWorkflow, WorkflowStep } from "~/automations/workflows";
import { useOpenIntegrationUrl } from "~/shared/integration";

export function GoogleDriveConfig({
  step,
  onChange,
}: {
  step: Extract<WorkflowStep, { type: "google_drive_export" }>;
  onChange: (step: WorkflowStep) => void;
}) {
  const client = useAuthedApiClient();
  const { openIntegration, openingAction } = useOpenIntegrationUrl();
  const picker = useMutation({
    mutationFn: async (connectionId: string) => {
      if (!client) throw new Error("Sign in to connect Google Drive");
      const folder = await pickDriveFolder(client, connectionId);
      if (folder) onChange({ ...step, connectionId, target: folder });
    },
  });
  return (
    <div className="flex flex-col items-start gap-2">
      <IntegrationGate
        integrationId="google-drive"
        connectLabel={<Trans>Connect Google Drive</Trans>}
        reconnectLabel={<Trans>Reconnect Google Drive</Trans>}
      >
        {(connection) => (
          <>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={picker.isPending}
              onClick={() => picker.mutate(connection.connection_id)}
            >
              {picker.isPending ? (
                <Trans>Choose a folder in your browser…</Trans>
              ) : step.target &&
                step.connectionId === connection.connection_id ? (
                step.target.name
              ) : (
                <Trans>Choose Google Drive folder</Trans>
              )}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="link"
              disabled={openingAction !== null || picker.isPending}
              onClick={() =>
                openIntegration({
                  nangoIntegrationId: "google-drive",
                  connectionId: connection.connection_id,
                  action: "reconnect",
                })
              }
            >
              <Trans>Reconnect Google Drive</Trans>
            </Button>
            {step.connectionId &&
            step.connectionId !== connection.connection_id ? (
              <p className="text-destructive text-xs">
                <Trans>
                  Your Google account changed. Choose a folder again.
                </Trans>
              </p>
            ) : null}
          </>
        )}
      </IntegrationGate>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Every note summarized after enabling this automation is saved here as
          one Markdown file with its summary and transcript. Anarlog must be
          open.
        </Trans>
      </p>
      {picker.error ? (
        <p role="alert" className="text-destructive text-xs">
          {picker.error.message}
        </p>
      ) : null}
    </div>
  );
}

export function DriveExportResult({
  workflow,
  step,
}: {
  workflow: AutomationWorkflow;
  step: Extract<WorkflowStep, { type: "google_drive_export" }>;
}) {
  const runs = (workflow.driveExports ?? [])
    .filter(
      (run) =>
        run.stepId === step.id &&
        run.connectionId === step.connectionId &&
        run.folderId === step.target?.id,
    )
    .sort((a, b) => b.at.localeCompare(a.at));
  const latest = runs.find((run) => run.status !== "success") ?? runs[0];
  const retry = useMutation({
    mutationFn: async () => {
      if (latest) await retryDriveExport(workflow.id, latest);
    },
  });
  if (!latest) return null;
  const url =
    latest.status === "success" && latest.fileId
      ? `https://drive.google.com/file/d/${encodeURIComponent(latest.fileId)}/view`
      : null;
  return (
    <div className="mt-2 flex flex-col items-start gap-2 text-xs">
      {url ? (
        <Button
          variant="link"
          size="sm"
          onClick={() => void openerCommands.openUrl(url, null)}
        >
          <Trans>Open exported file in Google Drive</Trans>
        </Button>
      ) : (
        <>
          <p className="text-destructive">
            {latest.detail || (
              <Trans>Google Drive export did not finish.</Trans>
            )}
          </p>
          <Button
            size="sm"
            variant="outline"
            disabled={retry.isPending || !workflow.enabled}
            onClick={() => retry.mutate()}
          >
            <Trans>Retry Google Drive export</Trans>
          </Button>
        </>
      )}
      {retry.error ? (
        <p role="alert" className="text-destructive">
          {retry.error.message}
        </p>
      ) : null}
    </div>
  );
}
