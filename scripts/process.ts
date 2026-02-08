import { type ChildProcess, spawn, type SpawnOptions } from "node:child_process";

export function spawnInherited(
  command: string,
  args: readonly string[],
  options: SpawnOptions = {},
): ChildProcess {
  return spawn(command, args, {
    stdio: "inherit",
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
