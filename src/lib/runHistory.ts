import type { PortEntry } from "./types";
import type { DisplayLaunchContext } from "../features/recovery/types";
import type { HistoricalCommand } from "../features/recovery/types";

export function usefulRecentCommands(commands: HistoricalCommand[]) {
  const runtimePath =
    /(?:^|[\\/])(?:\.nvm|node_modules|\.cache|\.next|_npx|\.rustup)(?:[\\/]|$)/;
  const score = (command: HistoricalCommand) =>
    command.active ? 2 : command.latestRun.state === "FAILED" ? 1 : 0;
  return commands
    .filter(
      (command) =>
        !runtimePath.test(
          command.launchContext.projectId ??
            command.launchContext.workingDirectory,
        ) &&
        (command.active
          ? !!command.launchContext.projectId
          : command.launchContext.recoverable),
    )
    .sort(
      (a, b) =>
        score(a) - score(b) ||
        Number(!!b.launchContext.projectId) -
          Number(!!a.launchContext.projectId) ||
        b.latestRun.startedAt - a.latestRun.startedAt,
    )
    .slice(0, 3);
}

// Project and port alone are not enough: another command may now own that port.
export function activeLaunch(
  context: DisplayLaunchContext | null | undefined,
  ports: PortEntry[],
) {
  if (!context) return undefined;
  return ports.find(
    (entry) =>
      entry.launch?.id === context.id ||
      (!!context.fingerprint &&
        entry.launch?.fingerprint === context.fingerprint),
  );
}
