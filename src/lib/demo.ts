import type { PortEntry } from "./types";
const now = Math.floor(Date.now() / 1000);
function entry(
  port: number,
  process: string,
  pid: number,
  age: number,
  command: string[],
  extra: Partial<PortEntry> = {},
): PortEntry {
  return {
    id: `TCP:127.0.0.1:${port}:${pid}`,
    port,
    protocol: "TCP",
    address: "127.0.0.1",
    pid,
    process,
    command,
    executable: `/usr/local/bin/${process}`,
    cwd: "/Users/developer/Projects/workspace",
    parentPid: 48100,
    user: "developer",
    startedAt: now - age,
    memory: 48 * 1024 * 1024,
    cpu: 0.2,
    system: false,
    protected: false,
    restartable: false,
    restartReason:
      "The original environment and process supervisor cannot be safely reproduced.",
    permissionLimited: false,
    ...extra,
  };
}
export const demoPorts: PortEntry[] = [
  entry(
    3000,
    "node",
    52932,
    134,
    ["node", "node_modules/next/dist/bin/next", "dev"],
    { memory: 184 * 1024 * 1024 },
  ),
  entry(3001, "node", 52934, 526, ["node", "server.js"]),
  entry(5173, "vite", 48291, 864, ["node", "node_modules/vite/bin/vite.js"], {
    memory: 92 * 1024 * 1024,
    cpu: 0.6,
  }),
  entry(5432, "postgres", 911, 266400, [
    "postgres",
    "-D",
    "/usr/local/var/postgres",
  ]),
  entry(6379, "redis-server", 1234, 266400, ["redis-server", "127.0.0.1:6379"]),
  entry(8000, "python3", 50128, 2520, ["python3", "-m", "http.server", "8000"]),
  entry(8080, "rust-api", 51042, 1080, ["target/debug/rust-api"], {
    address: "0.0.0.0",
  }),
  entry(9090, "com.docker.backend", 1842, 15840, ["com.docker.backend"], {
    address: "0.0.0.0",
  }),
  entry(27017, "mongod", 1320, 179280, [
    "mongod",
    "--config",
    "/usr/local/etc/mongod.conf",
  ]),
  entry(5353, "mDNSResponder", 301, 441600, ["mDNSResponder"], {
    id: "UDP:0.0.0.0:5353:301",
    protocol: "UDP",
    address: "0.0.0.0",
    system: true,
    protected: true,
    user: "_mdnsresponder",
    cwd: null,
    executable: "/usr/sbin/mDNSResponder",
  }),
  entry(5000, "ControlCenter", 420, 441600, ["ControlCenter"], {
    address: "0.0.0.0",
    system: true,
    protected: true,
    user: "developer",
  }),
  entry(7000, "ControlCenter", 420, 441600, ["ControlCenter"], {
    address: "0.0.0.0",
    system: true,
    protected: true,
    user: "developer",
  }),
];
