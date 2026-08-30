import assert from "node:assert/strict";
import test from "node:test";

import { ENTERPRISE_EVENTS } from "./enterprise.ts";
import {
  architectureLayers,
  certificationStatus,
  contractualDocs,
  pilotSteps,
  proofStatus,
  securityReviewAnswers,
  shipsToday,
  shipsWithPartners,
  subprocessors,
} from "./trust-center.ts";

test("answers the security-review questions buyers forward to IT", () => {
  const questions = securityReviewAnswers.map((item) => item.question);

  assert.deepEqual(questions, [
    "How is meeting content encrypted?",
    "Where does data live, and which jurisdiction applies?",
    "How long do you keep data?",
    "Do you train models on our meetings?",
    "Who are your subprocessors?",
    "Does a bot join our calls?",
  ]);

  for (const item of securityReviewAnswers) {
    assert.ok(item.summary.length > 40);
    assert.ok(item.detail.length > item.summary.length);
  }
});

test("lists processors without claiming they can read Cloud Sync content", () => {
  const names = subprocessors.map((item) => item.name).join(" ");

  assert.match(names, /SQLite Cloud/);
  assert.match(names, /Stripe/);
  assert.match(names, /Deepgram/);
  assert.match(names, /OpenRouter/);
  assert.match(names, /Nango/);

  const sync = subprocessors.find((item) => item.name === "SQLite Cloud");
  assert.match(sync?.receives ?? "", /encrypted/i);
});

test("does not claim certifications that are not complete", () => {
  assert.deepEqual(certificationStatus.claimed, []);
  assert.match(certificationStatus.planned, /does not claim/);
  assert.doesNotMatch(certificationStatus.planned, /we are SOC 2/i);
  assert.doesNotMatch(certificationStatus.planned, /HIPAA compliant/i);
  assert.match(certificationStatus.hostedTrustCenter, /separate site/);
  assert.doesNotMatch(
    `${certificationStatus.planned} ${certificationStatus.hostedTrustCenter}`,
    /this page is (the|our) trust center/i,
  );
});

test("keeps the DPA as a request, not a published legal invention", () => {
  const dpa = contractualDocs.find((doc) => doc.label.includes("Processing"));
  assert.ok(dpa?.href.startsWith("mailto:"));
  assert.match(dpa?.note ?? "", /on request/i);
});

test("describes a founder-led rollout without inventing customer proof", () => {
  assert.equal(pilotSteps.length, 3);
  assert.equal(pilotSteps[0]?.title, "Security review");
  assert.equal(pilotSteps[1]?.title, "Scoped pilot");
  assert.equal(pilotSteps[2]?.title, "Rollout");
  assert.match(proofStatus.body, /does not invent/);
  assert.ok(shipsToday.length >= 4);
  assert.ok(shipsWithPartners.length >= 2);
  assert.equal(architectureLayers.length, 4);
});

test("uses stable funnel event names", () => {
  assert.deepEqual(ENTERPRISE_EVENTS, {
    pageViewed: "enterprise_page_viewed",
    securityPageViewed: "security_page_viewed",
    ctaClicked: "enterprise_cta_clicked",
  });
});
