import { tool } from "ai";
import { z } from "zod";

import { liveQueryClient } from "~/db";
import { createNamedFolder } from "~/session/folder-catalog";
import { normalizeFolderPath } from "~/session/folders";
import { loadFolderPaths } from "~/session/queries/folders";
import { updateSession } from "~/session/queries/sessions";

const folderPathSchema = z
  .string()
  .refine(
    (value) => normalizeFolderPath(value) !== null,
    "Use a relative folder path without empty, . or .. segments",
  );

export const buildListFoldersTool = () =>
  tool({
    description:
      "List existing meeting folders, including empty folders and parent folders. Use their exact paths for folder operations. Query matches a case-insensitive path substring; follow next_offset for more results.",
    inputSchema: z.object({
      query: z.string().optional(),
      limit: z.number().int().min(1).max(200).optional(),
      offset: z.number().int().min(0).optional(),
    }),
    execute: async ({ query = "", limit = 100, offset = 0 }) => {
      const needle = query.trim().toLocaleLowerCase();
      const paths = (await loadFolderPaths()).filter((path) =>
        path.toLocaleLowerCase().includes(needle),
      );
      const folders = paths.slice(offset, offset + limit);
      return {
        folders,
        total: paths.length,
        next_offset:
          offset + folders.length < paths.length
            ? offset + folders.length
            : null,
      };
    },
  });

export const buildCreateFolderTool = () =>
  tool({
    description:
      "Create a named meeting folder, including parent folders for a nested path. List folders first and reuse an existing destination when it matches the user's request. This does not move any meetings.",
    inputSchema: z.object({
      folder_path: folderPathSchema.refine(
        (value) => Boolean(normalizeFolderPath(value)),
        "A named folder is required",
      ),
    }),
    execute: async ({ folder_path }) => ({
      status: "ok" as const,
      folder_path: await createNamedFolder(folder_path),
    }),
  });

async function loadMeetingFolder(meetingId: string) {
  const rows = await liveQueryClient.execute<{
    title: string;
    folder_path: string;
  }>(
    `SELECT title, folder_path FROM sessions WHERE id = ? AND deleted_at IS NULL`,
    [meetingId],
  );
  return rows[0];
}

export const buildMoveMeetingsToFolderTool = () =>
  tool({
    description:
      "Move up to 200 existing meetings into an existing folder, or use an empty folder_path to unfile them. This changes folder assignment only; it does not merge meetings or move contents between meetings. Resolve meeting IDs with list_meetings or get_recurring_meeting_history and use the exact destination from list_folders or create_folder. For all/every requests, follow meeting pagination and submit each batch. Results report moved, unchanged, and failed meetings separately; never claim failures succeeded.",
    inputSchema: z.object({
      meeting_ids: z.array(z.string().trim().min(1)).min(1).max(200),
      folder_path: folderPathSchema.describe(
        "Exact destination folder path, or an empty string to remove folder assignment",
      ),
    }),
    execute: async ({ meeting_ids, folder_path }, { abortSignal }) => {
      const path = normalizeFolderPath(folder_path);
      if (
        path === null ||
        (path && !(await loadFolderPaths()).includes(path))
      ) {
        return {
          status: "error" as const,
          message:
            "Destination folder does not exist. Use list_folders or create_folder first.",
          results: [],
        };
      }

      const results: Array<{
        meeting_id: string;
        title?: string;
        previous_folder_path?: string;
        status: "moved" | "unchanged" | "error";
        message?: string;
      }> = [];
      for (const meetingId of new Set(meeting_ids)) {
        try {
          abortSignal?.throwIfAborted();
          const meeting = await loadMeetingFolder(meetingId);
          if (!meeting) {
            results.push({
              meeting_id: meetingId,
              status: "error",
              message: "Meeting not found or deleted",
            });
            continue;
          }

          const unchanged = normalizeFolderPath(meeting.folder_path) === path;
          if (!unchanged) {
            abortSignal?.throwIfAborted();
            await updateSession(meetingId, { folder_id: path });
            const updated = await loadMeetingFolder(meetingId);
            if (!updated || updated.folder_path !== path) {
              throw new Error(
                "Meeting folder assignment could not be verified",
              );
            }
          }
          results.push({
            meeting_id: meetingId,
            title: meeting.title,
            previous_folder_path: meeting.folder_path,
            status: unchanged ? "unchanged" : "moved",
          });
        } catch (error) {
          results.push({
            meeting_id: meetingId,
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          });
        }
      }

      const failed = results.filter(
        (result) => result.status === "error",
      ).length;
      return {
        status:
          failed === 0
            ? ("ok" as const)
            : failed === results.length
              ? ("error" as const)
              : ("partial" as const),
        folder_path: path,
        moved: results.filter((result) => result.status === "moved").length,
        unchanged: results.filter((result) => result.status === "unchanged")
          .length,
        failed,
        results,
      };
    },
    toModelOutput: ({ output }) => ({
      type: "json",
      value: {
        ...output,
        results: output.results.filter((result) => result.status === "error"),
      },
    }),
  });
