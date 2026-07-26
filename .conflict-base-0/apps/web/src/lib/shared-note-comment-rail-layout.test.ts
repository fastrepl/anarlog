import assert from "node:assert/strict";
import test from "node:test";

import {
  layoutRailCards,
  pickActiveCommentId,
} from "./shared-note-comment-rail-layout.ts";

test("stacks cards without overlap in anchor order", () => {
  const placements = layoutRailCards(
    [
      { id: "a", desiredTop: 0, height: 80 },
      { id: "b", desiredTop: 20, height: 60 },
      { id: "c", desiredTop: 400, height: 40 },
    ],
    { gap: 12, activeId: null },
  );

  assert.deepEqual(
    placements.map((placement) => placement.id),
    ["a", "b", "c"],
  );
  assert.equal(placements[0].top, 0);
  assert.equal(placements[1].top, 92);
  assert.equal(placements[2].top, 400);
});

test("pins the active card at its desired top and pushes neighbors", () => {
  const placements = layoutRailCards(
    [
      { id: "a", desiredTop: 100, height: 80 },
      { id: "b", desiredTop: 110, height: 60 },
      { id: "c", desiredTop: 120, height: 40 },
    ],
    { gap: 10, activeId: "b" },
  );
  const byId = new Map(placements.map((p) => [p.id, p.top]));

  assert.equal(byId.get("b"), 110);
  assert.ok(byId.get("a")! + 80 + 10 <= byId.get("b")!);
  assert.ok(byId.get("c")! >= byId.get("b")! + 60 + 10);
});

test("normalizes when upward pushes go negative", () => {
  const placements = layoutRailCards(
    [
      { id: "a", desiredTop: 0, height: 100 },
      { id: "b", desiredTop: 10, height: 50 },
    ],
    { gap: 8, activeId: "b" },
  );
  for (const placement of placements) {
    assert.ok(placement.top >= 0);
  }
  const byId = new Map(placements.map((p) => [p.id, p.top]));
  assert.ok(byId.get("a")! + 100 + 8 <= byId.get("b")!);
});

test("orders deterministically for equal desired tops", () => {
  const placements = layoutRailCards(
    [
      { id: "z", desiredTop: 50, height: 30 },
      { id: "a", desiredTop: 50, height: 30 },
    ],
    { gap: 6, activeId: null },
  );
  assert.deepEqual(
    placements.map((placement) => placement.id),
    ["a", "z"],
  );
});

test("picks the smallest overlapping highlight", () => {
  const candidates = [
    { commentId: "outer", from: 1, to: 100 },
    { commentId: "inner", from: 10, to: 20 },
    { commentId: "other", from: 200, to: 210 },
  ];
  assert.equal(pickActiveCommentId(candidates, ["outer", "inner"]), "inner");
  assert.equal(pickActiveCommentId(candidates, ["outer"]), "outer");
  assert.equal(pickActiveCommentId(candidates, ["missing"]), null);
});
