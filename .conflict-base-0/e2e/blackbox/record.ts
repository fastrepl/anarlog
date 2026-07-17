import type { Frameworks } from "@wdio/types";
import { type ChildProcessWithoutNullStreams, spawn } from "child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";

function fileName(title: string) {
  return encodeURIComponent(title.trim().replace(/\s+/g, "-"));
}

export class TestRecorder {
  ffmpeg?: ChildProcessWithoutNullStreams;

  constructor() {}

  stop() {
    this.ffmpeg?.kill("SIGINT");
  }

  start(test: Frameworks.Test, videoPath: string) {
    if (!videoPath || !test) {
      throw new Error(
        "Cannot start recording without a test and path for the video file.",
      );
    }

    if (process.env.DISPLAY && process.env.DISPLAY.startsWith(":")) {
      mkdirSync(videoPath, { recursive: true });

      const parsedPath = path.join(
        videoPath,
        `${fileName(test.parent)}-${fileName(test.title)}.mp4`,
      );

      this.ffmpeg = spawn("ffmpeg", [
        "-f",
        "x11grab",
        "-video_size",
        "1160x720",
        "-i",
        process.env.DISPLAY,
        "-loglevel",
        "error",
        "-y",
        "-pix_fmt",
        "yuv420p",
        parsedPath,
      ]);

      function logBuffer(buffer: Buffer, prefix: string) {
        const lines = buffer.toString().trim().split("\n");
        lines.forEach(function (line) {
          console.log(prefix + line);
        });
      }

      this.ffmpeg.on("error", (err: NodeJS.ErrnoException) => {
        if (err.code === "ENOENT") {
          console.warn("[recorder] ffmpeg not found; skipping video recording");
        } else {
          console.warn(
            "[recorder] failed to start ffmpeg; skipping video recording:",
            err,
          );
        }
        this.ffmpeg = undefined;
      });

      this.ffmpeg.stdout.on("data", (data: Buffer) => {
        logBuffer(data, "[ffmpeg:stdout] ");
      });

      this.ffmpeg.stderr.on("data", (data: Buffer) => {
        logBuffer(data, "[ffmpeg:error] ");
      });

      this.ffmpeg.on("close", (code?: number, signal?: string) => {
        if (code) {
          console.log(`[ffmpeg:stdout] exited with code ${code}: ${videoPath}`);
        }
        if (signal) {
          console.log(
            `[ffmpeg:stdout] received signal ${signal}: ${videoPath}`,
          );
        }
      });
    }
  }
}
