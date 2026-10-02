import assert from "node:assert/strict";
import test from "node:test";

import {
  parseAttachmentBackupDownload,
  requestAttachmentBackupDownload,
} from "./attachment-backup-client.ts";

const objectId = "02425e17-c452-41c9-869c-196d09f75c91";
const objectKey = `e8d8149f-af6a-4c14-8b91-066fa196187c/${objectId}.anb1`;

test("accepts a matching attachment download grant", () => {
  assert.deepEqual(
    parseAttachmentBackupDownload(
      {
        objectId,
        objectKey,
        ciphertextSizeBytes: 123,
        ciphertextSha256: "a".repeat(64),
        formatVersion: 1,
        signedUrl: `https://project.supabase.co/storage/v1/object/sign/attachment-backups/${objectKey}?token=one`,
      },
      objectKey,
    ),
    {
      objectId,
      objectKey,
      ciphertextSizeBytes: 123,
      ciphertextSha256: "a".repeat(64),
      formatVersion: 1,
      signedUrl: `https://project.supabase.co/storage/v1/object/sign/attachment-backups/${objectKey}?token=one`,
    },
  );
});

test("rejects grants for another object or unsupported format", () => {
  const grant = {
    objectId,
    objectKey,
    ciphertextSizeBytes: 123,
    ciphertextSha256: "a".repeat(64),
    formatVersion: 1,
    signedUrl: "https://project.supabase.co/download?token=one",
  };

  assert.throws(
    () =>
      parseAttachmentBackupDownload(
        {
          ...grant,
          objectKey: `e8d8149f-af6a-4c14-8b91-066fa196187c/${crypto.randomUUID()}.anb1`,
        },
        objectKey,
      ),
    /invalid recording download/,
  );
  assert.throws(
    () =>
      parseAttachmentBackupDownload({ ...grant, formatVersion: 2 }, objectKey),
    /invalid recording download/,
  );
});

test("does not start an already cancelled download", async () => {
  const controller = new AbortController();
  controller.abort(new Error("cancelled before download"));

  await assert.rejects(
    requestAttachmentBackupDownload({
      accessToken: "token",
      apiBaseUrl: "https://api.anarlog.so",
      objectKey,
      signal: controller.signal,
    }),
    /cancelled before download/,
  );
});
