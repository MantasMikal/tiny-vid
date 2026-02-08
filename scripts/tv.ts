import { Command } from "commander";

import { runBuildCommand } from "./commands/build.ts";
import { runCleanBundleCommand } from "./commands/clean.ts";
import { runDevCommand } from "./commands/dev.ts";
import { runFfmpegBuildCommand, runFfmpegPrepareCommand } from "./commands/ffmpeg.ts";
import { runIconGenerateCommand } from "./commands/icon.ts";
import { runTestMatrixCommand, runTestSuiteCommand } from "./commands/test.ts";
import { type FfmpegProfileInput, normalizeFfmpegProfile } from "./ffmpeg-profile.ts";
import {
  dryRunOption,
  modeOption,
  platformOption,
  profileOption,
  suiteOption,
  verboseOption,
} from "./options.ts";
import { type CommandContext, configureShell, createContext } from "./runtime.ts";
import { type BuildMode, type BuildPlatform, type TestSuite } from "./standalone.ts";

interface CommonCliOptions {
  dryRun?: boolean;
  verbose?: boolean;
}

interface BuildCliOptions extends CommonCliOptions {
  mode: BuildMode;
  platform: BuildPlatform;
  profile?: FfmpegProfileInput;
}

interface DevCliOptions extends CommonCliOptions {
  mode: BuildMode;
  profile?: FfmpegProfileInput;
}

interface TestCliOptions extends CommonCliOptions {
  mode: BuildMode;
  profile?: FfmpegProfileInput;
  suite: TestSuite;
}

interface FfmpegCliOptions extends CommonCliOptions {
  profile: FfmpegProfileInput;
}

type CommandActionResult = Promise<number>;

type CliOptions = Record<string, unknown>;

function normalizeCommandOptions(rawArgs: unknown[]): CliOptions {
  const merged: Record<string, unknown> = {};

  for (const raw of rawArgs) {
    if (!raw || typeof raw !== "object") {
      continue;
    }

    const maybeWithOpts = raw as { opts?: () => unknown };
    if (typeof maybeWithOpts.opts === "function") {
      const parsed = maybeWithOpts.opts();
      if (parsed && typeof parsed === "object") {
        Object.assign(merged, parsed as Record<string, unknown>);
      }
    }

    Object.assign(merged, raw as Record<string, unknown>);
  }

  return merged;
}

async function runWithContext(
  options: CommonCliOptions,
  action: (context: CommandContext) => CommandActionResult
): Promise<void> {
  const argvDryRun = process.argv.includes("--dry-run");
  const argvVerbose = process.argv.includes("--verbose");
  const globalOptions = {
    dryRun: options.dryRun ?? argvDryRun,
    verbose: options.verbose ?? argvVerbose,
  };

  configureShell(globalOptions.verbose);
  const context = createContext(globalOptions);
  const status = await action(context);
  if (status !== 0) {
    process.exitCode = status;
  }
}

function withContextAction(
  action: (context: CommandContext, options: CliOptions) => CommandActionResult
): (...rawArgs: unknown[]) => Promise<void> {
  return async (...rawArgs: unknown[]) => {
    const options = normalizeCommandOptions(rawArgs);
    await runWithContext(options as CommonCliOptions, (context) => action(context, options));
  };
}

function createProgram(): Command {
  const program = new Command();

  program.name("tv");
  program.description("Tiny Vid script runner");
  program.showHelpAfterError();
  program.showSuggestionAfterError(true);

  program
    .command("build")
    .description("Build Electron app bundles")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .addOption(modeOption())
    .addOption(profileOption())
    .addOption(platformOption())
    .action(
      withContextAction((ctx, rawOptions) => {
        const opts = rawOptions as BuildCliOptions;
        return runBuildCommand(ctx, {
          mode: opts.mode,
          profile: normalizeFfmpegProfile(opts.profile),
          platform: opts.platform,
        });
      })
    );

  program
    .command("dev")
    .description("Run Electron dev")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .addOption(modeOption())
    .addOption(profileOption())
    .action(
      withContextAction((ctx, rawOptions) => {
        const opts = rawOptions as DevCliOptions;
        return runDevCommand(ctx, {
          mode: opts.mode,
          profile: normalizeFfmpegProfile(opts.profile),
        });
      })
    );

  const testCmd = program
    .command("test")
    .description("Run Rust test suites")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .addOption(modeOption())
    .addOption(profileOption())
    .addOption(suiteOption())
    .action(
      withContextAction((ctx, rawOptions) => {
        const opts = rawOptions as TestCliOptions;
        return runTestSuiteCommand(ctx, {
          mode: opts.mode,
          profile: normalizeFfmpegProfile(opts.profile),
          suite: opts.suite,
        });
      })
    );

  testCmd
    .command("matrix")
    .description("Run every supported test combination")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction((ctx) => runTestMatrixCommand(ctx)));

  program
    .command("ffmpeg")
    .description("FFmpeg profile commands")
    .addCommand(
      new Command("prepare")
        .description("Prepare profile binaries")
        .addOption(dryRunOption())
        .addOption(verboseOption())
        .addOption(profileOption(true))
        .action(
          withContextAction((ctx, rawOptions) => {
            const opts = rawOptions as FfmpegCliOptions;
            const profile = normalizeFfmpegProfile(opts.profile);
            if (!profile) {
              throw new Error("Missing --profile");
            }
            return runFfmpegPrepareCommand(ctx, { profile });
          })
        )
    )
    .addCommand(
      new Command("build")
        .description("Build profile binaries from source")
        .addOption(dryRunOption())
        .addOption(verboseOption())
        .addOption(profileOption(true))
        .action(
          withContextAction((ctx, rawOptions) => {
            const opts = rawOptions as FfmpegCliOptions;
            const profile = normalizeFfmpegProfile(opts.profile);
            if (!profile) {
              throw new Error("Missing --profile");
            }
            return runFfmpegBuildCommand(ctx, { profile });
          })
        )
    );

  program
    .command("clean")
    .description("Cleanup commands")
    .command("bundle")
    .description("Remove Electron package output and bundled sidecar resources")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction((ctx) => runCleanBundleCommand(ctx)));

  program
    .command("icon")
    .description("Generate Electron app icons from app-icon.png")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction((ctx) => runIconGenerateCommand(ctx)));

  return program;
}

async function main(): Promise<void> {
  const program = createProgram();
  if (process.argv.length <= 2) {
    program.help();
  }
  await program.parseAsync(process.argv);
}

void main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
