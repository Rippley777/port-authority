import test from "node:test";
import assert from "node:assert/strict";
import { parseFavorites, parseHistory, parseSettings } from "./persistence";
import { defaultSettings } from "./types";
test("corrupt preferences fall back to safe refresh and confirmation defaults", () => {
  assert.deepEqual(parseSettings(null), defaultSettings);
  const parsed = parseSettings({
    refreshInterval: 0,
    confirmKill: "no",
    protocol: "file",
    retention: Infinity,
  });
  assert.equal(parsed.refreshInterval, 2);
  assert.equal(parsed.confirmKill, true);
  assert.equal(parsed.protocol, "http");
  assert.equal(parsed.retention, 500);
});
test("favorites reject invalid ports and deduplicate persisted data", () => {
  assert.deepEqual(
    parseFavorites([3000, "5173", 3000, 0, -1, 65536, 5432]),
    [3000, 5432],
  );
});
test("history ignores malformed events and bounds persistence", () => {
  const event = {
    id: "1",
    time: Date.now(),
    port: 3000,
    process: "node",
    type: "started",
  };
  assert.deepEqual(
    parseHistory([null, {}, event, { ...event, type: "injected" }]),
    [event],
  );
  assert.equal(parseHistory(Array(1200).fill(event)).length, 1000);
});
