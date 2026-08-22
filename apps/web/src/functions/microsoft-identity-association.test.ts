import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const PRODUCTION_OUTLOOK_APP_ID = "22bc6342-cd6a-4784-8d66-3242113fe7fb";
const ANARLOG_OUTLOOK_APP_ID = "3d7c41ea-a814-489f-9d57-9a4bbe780ddb";

test("publisher-domain association keeps both Outlook Entra apps", () => {
  const association = JSON.parse(
    readFileSync(
      join(
        dirname(fileURLToPath(import.meta.url)),
        "../../public/.well-known/microsoft-identity-association.json",
      ),
      "utf8",
    ),
  ) as { associatedApplications?: { applicationId?: string }[] };

  const applicationIds = (association.associatedApplications ?? []).map(
    (application) => application.applicationId,
  );

  assert.deepEqual(applicationIds, [
    PRODUCTION_OUTLOOK_APP_ID,
    ANARLOG_OUTLOOK_APP_ID,
  ]);
});
