import * as Sentry from "@sentry/react";
import {
  type ErrorRouteComponent,
  NotFoundRouteComponent,
  useNavigate,
} from "@tanstack/react-router";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { arch, version as osVersion, platform } from "@tauri-apps/plugin-os";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  AlertTriangle,
  Bug,
  Home,
  Loader2,
  RotateCw,
  Search,
} from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState } from "react";

import { commands as miscCommands } from "@hypr/plugin-misc";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { commands as tracingCommands } from "@hypr/plugin-tracing";
import { Button } from "@hypr/ui/components/ui/button";

import { env } from "~/env";

export const ErrorComponent: ErrorRouteComponent = ({ error }) => {
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    Sentry.captureException(error);
  }, [error]);

  const handleRestart = async () => {
    try {
      await relaunch();
    } catch (err) {
      console.error("Failed to restart app:", err);
    }
  };

  const handleReportIssue = async () => {
    if (isSubmitting) return;

    setIsSubmitting(true);

    try {
      const gitHashResult = await miscCommands.getGitHash();
      const gitHash =
        gitHashResult.status === "ok" ? gitHashResult.data : "unknown";

      let logs: string | undefined;
      const logsResult = await tracingCommands.logContent();
      if (logsResult.status === "ok" && logsResult.data) {
        logs = logsResult.data.slice(-10000);
      }

      const errorDetails = [
        `**Error Message:** ${error.message || "Unknown error"}`,
        error.stack ? `**Stack Trace:**\n\`\`\`\n${error.stack}\n\`\`\`` : "",
      ]
        .filter(Boolean)
        .join("\n\n");

      const description = `## Error Details\n${errorDetails}`;

      const response = await tauriFetch(`${env.VITE_API_URL}/feedback/submit`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          type: "bug",
          description,
          logs,
          deviceInfo: {
            platform: platform(),
            arch: arch(),
            osVersion: osVersion(),
            appVersion: env.VITE_APP_VERSION ?? "unknown",
            gitHash,
          },
        }),
      });

      const data = (await response.json()) as {
        success: boolean;
        issueUrl?: string;
        error?: string;
      };

      if (data.success && data.issueUrl) {
        await openerCommands.openUrl(data.issueUrl, null);
      } else {
        console.error("Failed to submit error report:", data.error);
      }
    } catch (err) {
      console.error(
        "Failed to submit error report:",
        err instanceof Error ? err.message : String(err),
      );
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="flex h-full flex-col">
      <div
        data-tauri-drag-region
        className="fixed inset-x-0 top-0 z-50 h-10 bg-transparent"
      />

      <div className="flex h-full min-h-[300px] items-center justify-center p-6">
        <motion.div
          className="w-full max-w-sm"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div className="rounded-xl border border-neutral-200 bg-white p-6 shadow-xs">
            <div className="flex flex-col items-center gap-4 text-center">
              <motion.div
                className="flex h-12 w-12 items-center justify-center rounded-full bg-red-50"
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <AlertTriangle className="h-6 w-6 text-red-500" />
              </motion.div>

              <div className="flex flex-col gap-1.5">
                <h2 className="text-base font-semibold text-neutral-900">
                  Something went wrong
                </h2>
                <p className="max-w-[260px] text-sm leading-relaxed text-neutral-500">
                  {error.message || "An unexpected error occurred."}
                </p>
              </div>

              <div className="flex gap-2 pt-2">
                <Button size="sm" onClick={handleRestart}>
                  <RotateCw className="mr-1.5 h-3.5 w-3.5" />
                  Restart App
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleReportIssue}
                  disabled={isSubmitting}
                >
                  {isSubmitting ? (
                    <>
                      <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                      Submitting...
                    </>
                  ) : (
                    <>
                      <Bug className="mr-1.5 h-3.5 w-3.5" />
                      Report Issue
                    </>
                  )}
                </Button>
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
};

export const NotFoundComponent: NotFoundRouteComponent = () => {
  const navigate = useNavigate();

  return (
    <div className="flex h-full flex-col">
      <div
        data-tauri-drag-region
        className="fixed inset-x-0 top-0 z-50 h-10 bg-transparent"
      />

      <div className="flex h-full min-h-[300px] items-center justify-center p-6">
        <motion.div
          className="w-full max-w-sm"
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: "easeOut" }}
        >
          <div className="rounded-xl border border-neutral-200 bg-white p-6 shadow-xs">
            <div className="flex flex-col items-center gap-4 text-center">
              <motion.div
                className="flex h-12 w-12 items-center justify-center rounded-full bg-neutral-100"
                initial={{ scale: 0.8 }}
                animate={{ scale: 1 }}
                transition={{
                  delay: 0.1,
                  type: "spring",
                  stiffness: 200,
                }}
              >
                <Search className="h-6 w-6 text-neutral-400" />
              </motion.div>

              <div className="flex flex-col gap-1.5">
                <motion.span
                  className="block text-4xl font-bold text-neutral-300"
                  initial={{ scale: 0.9, opacity: 0 }}
                  animate={{ scale: 1, opacity: 1 }}
                  transition={{
                    delay: 0.15,
                    type: "spring",
                    stiffness: 200,
                  }}
                >
                  404
                </motion.span>
                <h2 className="text-base font-semibold text-neutral-900">
                  Page not found
                </h2>
                <p className="text-sm text-neutral-500">
                  The page you're looking for doesn't exist.
                </p>
              </div>

              <div className="pt-2">
                <Button size="sm" onClick={() => navigate({ to: "/app/main" })}>
                  <Home className="mr-1.5 h-3.5 w-3.5" />
                  Go to Home
                </Button>
              </div>
            </div>
          </div>
        </motion.div>
      </div>
    </div>
  );
};
