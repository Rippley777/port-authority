import test from "node:test";
import assert from "node:assert/strict";
import { demoPorts } from "./demo";
import {
  connectionAddress,
  filterPorts,
  isHttp,
  isLocal,
  portFromQuery,
  reconcile,
  service,
  sortPorts,
} from "./ports";
test("validates exact port queries and rejects invalid port ranges", () => {
  assert.equal(portFromQuery(":5173"), 5173);
  assert.equal(portFromQuery(" 65535 "), 65535);
  for (const input of ["0", "65536", "-1", "1.5", "node", "12x", ""])
    assert.equal(portFromQuery(input), null);
});
test("exact search does not match similar port numbers or process IDs", () => {
  const p = demoPorts.find((p) => p.port === 5173)!;
  const decoy = { ...p, id: "decoy", port: 15173, pid: 5173 };
  assert.deepEqual(filterPorts([p, decoy], ":5173", "All", true), [p]);
  assert.equal(filterPorts([p, decoy], "5173", "All", true).length, 2);
});
test("searches process metadata, localhost aliases, and PIDs", () => {
  assert.equal(filterPorts(demoPorts, "next", "All", true)[0].port, 3000);
  assert.equal(filterPorts(demoPorts, "48291", "All", true)[0].process, "vite");
  assert(
    filterPorts(demoPorts, "localhost", "All", true).every((p) =>
      isLocal(p.address),
    ),
  );
});
test("combines protocol, address scope, and system visibility", () => {
  assert.equal(filterPorts(demoPorts, "", "UDP", false).length, 0);
  assert.equal(filterPorts(demoPorts, "", "UDP", true).length, 1);
  assert(
    filterPorts(demoPorts, "", "External", true).every(
      (p) => !isLocal(p.address),
    ),
  );
  assert(
    filterPorts(demoPorts, "", "Listening", true).every(
      (p) => p.protocol === "TCP",
    ),
  );
});
test("service recognition uses process metadata, not a port-number guess", () => {
  const p = { ...demoPorts[0], port: 5173, process: "unknown", command: [] };
  assert.equal(service(p).label, "Local service");
  assert.equal(service(demoPorts[2]).label, "Vite dev server");
});
test("HTTP suggestions exclude UDP and non-HTTP common services", () => {
  assert.equal(isHttp(demoPorts[2]), true);
  assert.equal(isHttp({ ...demoPorts[2], protocol: "UDP" }), false);
  assert.equal(isHttp(demoPorts[3]), false);
});
test("sorts numerically without mutating the source array", () => {
  const original = [...demoPorts];
  const sorted = sortPorts(original, "port", true);
  assert.equal(sorted[0].port, 3000);
  assert.equal(sorted.at(-1)?.port, 27017);
  assert.deepEqual(original, demoPorts);
});
test("retains object identity for unchanged rows across scans", () => {
  const next = demoPorts.map((p) => ({ ...p }));
  next[0] = { ...next[0], cpu: 5 };
  const result = reconcile(demoPorts, next);
  assert.notEqual(result[0], demoPorts[0]);
  assert.equal(result[1], demoPorts[1]);
});
test("formats IPv6 connection addresses unambiguously", () => {
  assert.equal(
    connectionAddress({ ...demoPorts[0], address: "::1" }),
    "[::1]:3000",
  );
});
