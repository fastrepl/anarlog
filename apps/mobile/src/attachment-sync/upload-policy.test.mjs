import assert from "node:assert/strict";
import test from "node:test";

import {
  attachmentUploadRetryDelayMs,
  isPermanentNativeUploadMessage,
  isPrivateAttachmentIdentity,
} from "./upload-policy.ts";

test("backs attachment upload retries off to a bounded ceiling", () => {
  assert.equal(attachmentUploadRetryDelayMs(1), 5_000);
  assert.equal(attachmentUploadRetryDelayMs(4), 40_000);
  assert.equal(attachmentUploadRetryDelayMs(20), 15 * 60 * 1000);
});

test("gives authorization failures a slower retry window", () => {
  assert.equal(attachmentUploadRetryDelayMs(1, 403), 5 * 60 * 1000);
  assert.equal(attachmentUploadRetryDelayMs(2, 403), 10 * 60 * 1000);
});

test("treats local integrity and path failures as permanent", () => {
  assert.equal(isPermanentNativeUploadMessage("checksum mismatch"), true);
  assert.equal(isPermanentNativeUploadMessage("source path is invalid"), true);
  assert.equal(isPermanentNativeUploadMessage("network unavailable"), false);
  assert.equal(
    isPermanentNativeUploadMessage("resource temporarily unavailable"),
    false,
  );
});

test("binds the reserved object id to its private object key", () => {
  const owner = "550e8400-e29b-41d4-a716-446655440000";
  const objectId = "9cfb2f08-a02f-41cf-a13e-07f36b87ef2b";
  assert.equal(
    isPrivateAttachmentIdentity(objectId, `${owner}/${objectId}.anb1`),
    true,
  );
  assert.equal(
    isPrivateAttachmentIdentity(
      objectId,
      `${owner}/7911712f-122e-4ff3-9517-c3f099bdeee6.anb1`,
    ),
    false,
  );
  assert.equal(isPrivateAttachmentIdentity("not-a-uuid", "bad/key"), false);
});
