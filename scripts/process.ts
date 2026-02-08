import { type ChildProcess, spawn, type SpawnOptions } from "node:child_process";

export function spawnInherited(
  command: string,
  args: readonly string[],
  options: SpawnOptions = {},
): ChildProcess {
  const isWindows = process.platform === "win32";
  const needsShell =
    isWindows &&
    (command.endsWith(".cmd") || command.endsWith(".bat"));
  return spawn(command, args, {
    stdio: "inherit",
    ...(needsShell && { shell: true }),
    ...options,
  });
}

export function runInheritedCommand(
  command: string,
  args: readonly string[],
  options: SpawnOptions = {},
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawnInherited(command, args, options);
    child.on("error", (error) => {
      reject(error);
    });
    child.on("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(
        new Error(
          `${command} ${args.join(" ")} exited with code=${String(code)} signal=${String(signal)}`,
        ),
      );
    });
  });
}
