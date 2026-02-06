import { Command } from "commander";

import { runBuildCommand } from "./commands/build.ts";
import { runCleanBundleCommand } from "./commands/clean.ts";
import { runDevCommand } from "./commands/dev.ts";
import { runFfmpegBuildCommand, runFfmpegPrepareCommand } from "./commands/ffmpeg.ts";
import { runIconCompileCommand } from "./commands/icon.ts";
import { runTestMatrixCommand, runTestSuiteCommand } from "./commands/test.ts";
import { type FfmpegProfile } from "./ffmpeg-profile.ts";
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
  profile?: FfmpegProfile;
}

interface DevCliOptions extends CommonCliOptions {
  mode: BuildMode;
  profile?: FfmpegProfile;
}

interface TestCliOptions extends CommonCliOptions {
  mode: BuildMode;
  profile?: FfmpegProfile;
  suite: TestSuite;
}

interface FfmpegCliOptions extends CommonCliOptions {
  profile: FfmpegProfile;
}

type CommandActionResult = Promise<number>;

async function runWithContext(
  options: CommonCliOptions,
  action: (context: CommandContext) => CommandActionResult
): Promise<void> {
  const globalOptions = {
    dryRun: options.dryRun ?? false,
    verbose: options.verbose ?? false,
  };

  configureShell(globalOptions.verbose);
  const context = createContext(globalOptions);
  const status = await action(context);
  if (status !== 0) {
    process.exitCode = status;
  }
}

function withContextAction<T extends CommonCliOptions>(
  action: (context: CommandContext, options: T) => CommandActionResult
): (options: T) => Promise<void> {
  return async (options: T) => {
    await runWithContext(options, (context) => action(context, options));
  };
}

function createProgram(): Command {
  const program = new Command();

  program
    .name("tv")
    .description("tiny-vid script runner")
    .showHelpAfterError()
    .showSuggestionAfterError(true);

  program
    .command("build")
    .description("Build app bundles")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .addOption(modeOption())
    .addOption(profileOption())
    .addOption(platformOption())
    .action(
      withContextAction<BuildCliOptions>((ctx, opts) =>
        runBuildCommand(ctx, {
          mode: opts.mode,
          profile: opts.profile,
          platform: opts.platform,
        })
      )
    );

  program
    .command("dev")
    .description("Run tauri dev")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .addOption(modeOption())
    .addOption(profileOption())
    .action(
      withContextAction<DevCliOptions>((ctx, opts) =>
        runDevCommand(ctx, { mode: opts.mode, profile: opts.profile })
      )
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
      withContextAction<TestCliOptions>((ctx, opts) =>
        runTestSuiteCommand(ctx, {
          mode: opts.mode,
          profile: opts.profile,
          suite: opts.suite,
        })
      )
    );

  testCmd
    .command("matrix")
    .description("Run every possible test combination (system + discovery, system + all, standalone + profile + all for each supported profile)")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction<CommonCliOptions>((ctx) => runTestMatrixCommand(ctx)));

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
          withContextAction<FfmpegCliOptions>((ctx, opts) =>
            runFfmpegPrepareCommand(ctx, { profile: opts.profile })
          )
        )
    )
    .addCommand(
      new Command("build")
        .description("Build profile binaries from source")
        .addOption(dryRunOption())
        .addOption(verboseOption())
        .addOption(profileOption(true))
        .action(
          withContextAction<FfmpegCliOptions>((ctx, opts) =>
            runFfmpegBuildCommand(ctx, { profile: opts.profile })
          )
        )
    );

  program
    .command("clean")
    .description("Cleanup commands")
    .command("bundle")
    .description("Remove src-tauri bundle output")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction<CommonCliOptions>((ctx) => runCleanBundleCommand(ctx)));

  program
    .command("icon")
    .description("Icon commands")
    .command("compile")
    .description("Compile macOS Assets.car from AppIcon.icon")
    .addOption(dryRunOption())
    .addOption(verboseOption())
    .action(withContextAction<CommonCliOptions>((ctx) => runIconCompileCommand(ctx)));

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
