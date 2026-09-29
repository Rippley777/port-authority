import test from "node:test";
import assert from "node:assert/strict";
import { ownershipSessions } from "./ownership";
import { demoPorts } from "./demo";
import type {
  TimelineEvent,
  MonitoringSession,
} from "../features/timeline/types";
const p = { process: demoPorts[0], ancestors: [] };
const base: TimelineEvent = {
  id: "claim",
  sequence: 1,
  timestamp: 10,
  sessionId: "s",
  port: p.process.port,
  protocol: "TCP",
  address: "127.0.0.1",
  eventType: "PORT_CLAIMED",
  process: p,
  previousProcess: null,
  uncertain: false,
  correlationId: null,
};
const session: MonitoringSession = {
  id: "s",
  startedAt: 10,
  lastObserved: 50,
  endedAt: 50,
  reason: "stopped",
};
test("ownership intervals preserve observed availability and stop at monitoring gaps", () => {
  const release: TimelineEvent = {
    ...base,
    id: "release",
    sequence: 2,
    timestamp: 20,
    eventType: "PORT_RELEASED",
    process: null,
    previousProcess: p,
    endpointAvailable: true,
  };
  const restart: TimelineEvent = {
    ...base,
    id: "restart",
    sequence: 3,
    timestamp: 30,
    eventType: "PROCESS_RESTARTED",
    previousProcess: p,
    process: { ...p, process: { ...p.process, pid: 999, startedAt: 30 } },
  };
  const result = ownershipSessions([restart, release, base], [session]);
  assert.equal(result.length, 3);
  assert.deepEqual(
    result.map((s) => [s.startedAt, s.endedAt, !!s.owner]),
    [
      [30, 50, true],
      [20, 30, false],
      [10, 20, true],
    ],
  );
  const next = {
    ...base,
    id: "resumed",
    sequence: 4,
    timestamp: 500,
    sessionId: "next",
    uncertain: true,
  };
  const afterGap = ownershipSessions(
    [next, restart, release, base],
    [session, { ...session, id: "next", startedAt: 500, lastObserved: 510 }],
  );
  assert.equal(afterGap[0].startedAt, 500);
  assert.equal(afterGap[1].endedAt, 50);
});
test("releasing one shared binding does not imply availability", () => {
  const release: TimelineEvent = {
    ...base,
    id: "release",
    sequence: 2,
    timestamp: 20,
    eventType: "PORT_RELEASED",
    process: null,
    previousProcess: p,
    endpointAvailable: false,
  };
  const result = ownershipSessions([base, release], [session]);
  assert.equal(result.length, 1);
  assert.ok(result[0].owner);
});
test("clock rollback never produces a negative interval", () => {
  const release: TimelineEvent = {
    ...base,
    id: "release",
    sequence: 2,
    timestamp: 5,
    eventType: "PORT_RELEASED",
    process: null,
    previousProcess: p,
  };
  assert.equal(ownershipSessions([base, release], [session])[0].endedAt, 10);
});
