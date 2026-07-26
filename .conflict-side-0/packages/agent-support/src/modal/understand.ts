import { REPO_PATH, runInSandbox } from "./sandbox";

export interface UnderstandResult {
  success: boolean;
  report: string;
  executionTimeMs: number;
}

export async function understandHyprnoteRepo(
  request: string,
): Promise<UnderstandResult> {
  try {
    const result = await runInSandbox(
      { timeoutMs: 5 * 60 * 1000 },
      async (sandbox) => {
        const setupScript = `
mkdir -p ~/.claude
printf '#!/bin/bash\\necho "$ANTHROPIC_API_KEY"\\n' > ~/.claude/anthropic_key.sh
chmod +x ~/.claude/anthropic_key.sh
cat > ~/.claude/settings.json << 'EOF'
{
  "apiKeyHelper": "~/.claude/anthropic_key.sh",
  "model": "opus",
  "alwaysThinkingEnabled": true
}
EOF
`;
        await sandbox.exec(["bash", "-c", setupScript], {
          stdout: "pipe",
          stderr: "pipe",
        });
        const claudeProcess = await sandbox.exec(
          [
            "claude",
            "-p",
            request,
            "--allowedTools",
            "Read,Grep,Glob,LS",
            "--output-format",
            "text",
          ],
          {
            stdout: "pipe",
            stderr: "pipe",
            workdir: REPO_PATH,
          },
        );
        const [stdout, stderr] = await Promise.all([
          claudeProcess.stdout.readText(),
          claudeProcess.stderr.readText(),
        ]);

        const exitCode = await claudeProcess.wait();
        const parts: string[] = [];
        if (stdout.trim()) parts.push(`stdout:\n${stdout.trim()}`);
        if (stderr.trim()) parts.push(`stderr:\n${stderr.trim()}`);
        console.log("Parts:", parts);
        return {
          success: exitCode === 0,
          data: {
            report: parts.length > 0 ? parts.join("\n\n") : "No output",
          },
        };
      },
    );

    return {
      success: result.success,
      report: result.data.report,
      executionTimeMs: result.executionTimeMs,
    };
  } catch (error) {
    const executionTimeMs =
      (error as { executionTimeMs?: number }).executionTimeMs ?? 0;
    return {
      success: false,
      report: error instanceof Error ? error.message : String(error),
      executionTimeMs,
    };
  }
}
