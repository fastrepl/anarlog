import { tool } from "ai";
import { z } from "zod";

import { commands as fsSyncCommands } from "@anlg/plugin-fs-sync";

import { CONTEXT_TEXT_FIELD } from "./context-text";
import { extractPdfText, isPdfMaterial } from "./pdf-text";
import type { ToolDependencies } from "./types";

import {
  diskAttachmentId,
  loadFolderMaterial,
} from "~/session/folder-attachments";

const DEFAULT_READ_MAX_CHARS = 16_000;
const MAX_READ_CHARS = 30_000;

const maxCharsSchema = z
  .number()
  .int()
  .min(1_000)
  .max(MAX_READ_CHARS)
  .optional()
  .describe("Maximum material characters to return to the model");

function clampMaxChars(value: number | undefined): number {
  return Math.min(
    Math.max(value ?? DEFAULT_READ_MAX_CHARS, 1_000),
    MAX_READ_CHARS,
  );
}

function isReadableText(contentType: string, filename: string): boolean {
  if (contentType.startsWith("text/")) {
    return true;
  }
  if (
    contentType === "application/json" ||
    contentType === "application/xml" ||
    contentType === "application/javascript"
  ) {
    return true;
  }

  return /\.(txt|md|markdown|csv|json|xml|html|css|js|ts|tsx)$/i.test(filename);
}

function decodeUtf8(bytes: number[]): string {
  return new TextDecoder("utf-8", { fatal: false }).decode(
    Uint8Array.from(bytes),
  );
}

function limitText(text: string, maxChars: number) {
  if (text.length <= maxChars) {
    return { text, truncated: false };
  }

  return {
    text: `${text.slice(0, maxChars).trimEnd()}\n\n[Content truncated]`,
    truncated: true,
  };
}

export const buildReadFolderMaterialTool = (deps: ToolDependencies) =>
  tool({
    description:
      "Read a folder material such as a syllabus, PDF, or text file attached to the current folder. Use the material IDs listed in folder context.",
    inputSchema: z.object({
      materialId: z
        .string()
        .describe("Folder material id from the folder context list"),
      maxChars: maxCharsSchema,
    }),
    execute: async (params: { materialId: string; maxChars?: number }) => {
      const folderPath = deps.getFolderFilter?.() ?? null;
      if (!folderPath) {
        return {
          status: "error" as const,
          message: "No folder is selected",
        };
      }

      const material = await loadFolderMaterial(folderPath, params.materialId);
      if (!material) {
        return {
          status: "error" as const,
          message: `Could not find folder material ${params.materialId}`,
          materialId: params.materialId,
        };
      }

      const attachmentId = diskAttachmentId(material.relativePath);
      const result = await fsSyncCommands.folderAttachmentRead(
        folderPath,
        attachmentId,
      );
      if (result.status === "error") {
        return {
          status: "error" as const,
          message: result.error,
          materialId: material.id,
          filename: material.filename,
        };
      }

      if (isPdfMaterial(material.contentType, material.filename)) {
        const extracted = await extractPdfText(result.data);
        if (extracted) {
          const limited = limitText(extracted, clampMaxChars(params.maxChars));
          return {
            status: "ok" as const,
            materialId: material.id,
            filename: material.filename,
            contentType: material.contentType,
            sizeBytes: material.sizeBytes,
            readable: true,
            truncated: limited.truncated,
            [CONTEXT_TEXT_FIELD]: limited.text,
          };
        }

        return {
          status: "ok" as const,
          materialId: material.id,
          filename: material.filename,
          contentType: material.contentType,
          sizeBytes: material.sizeBytes,
          readable: false,
          message:
            "Could not extract text from this PDF. Use the filename and folder notes.",
        };
      }

      if (!isReadableText(material.contentType, material.filename)) {
        return {
          status: "ok" as const,
          materialId: material.id,
          filename: material.filename,
          contentType: material.contentType,
          sizeBytes: material.sizeBytes,
          readable: false,
          message:
            "This file is binary. Text extraction is not available yet; use the filename and folder notes.",
        };
      }

      const limited = limitText(
        decodeUtf8(result.data),
        clampMaxChars(params.maxChars),
      );
      return {
        status: "ok" as const,
        materialId: material.id,
        filename: material.filename,
        contentType: material.contentType,
        sizeBytes: material.sizeBytes,
        readable: true,
        truncated: limited.truncated,
        [CONTEXT_TEXT_FIELD]: limited.text,
      };
    },
  });

export const folderMaterialTestInternals = {
  isReadableText,
  decodeUtf8,
  limitText,
};
