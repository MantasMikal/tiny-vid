import {
  FFMPEG_PROFILES,
  type FfmpegProfile,
  getTargetTriple,
  isMacOsTarget,
} from "../ffmpeg-profile.ts";
import { type CommandContext, runCommand } from "../runtime.ts";
import {
  assertStandaloneProfile,
  type BuildMode,
  setupStandaloneEnv,
  type TestSuite,
  validateStandaloneOptions,
} from "../standalone.ts";

export interface TestCommandOptions {
  mode: BuildMode;
  profile?: FfmpegProfile;
  suite: TestSuite;
}

function cargoBaseArgs(profile: FfmpegProfile | null): string[] {
  if (profile === "lgpl-vt") {
    return ["test", "--manifest-path", "src-tauri/Cargo.toml", "--features", "lgpl"];
  }

  return ["test", "--manifest-path", "src-tauri/Cargo.toml"];
}

const DISCOVERY_TEST_ARGS = [
  "test",
  "--manifest-path",
  "src-tauri/Cargo.toml",
  "--test",
  "discovery_bundled",
  "--features",
  "discovery-test-helpers",
  "--",
  "--test-threads=1",
] as const;

async function runTestSuite(
  context: CommandContext,
  suite: TestSuite,
  profile: FfmpegProfile | null,
  env: NodeJS.ProcessEnv
): Promise<void> {
  if (suite === "discovery") {
    await runCommand(context, "cargo", [...DISCOVERY_TEST_ARGS], { env });
    return;
  }

  const base = cargoBaseArgs(profile);
  const runUnit = suite === "unit" || suite === "all";
  const runIgnored = suite === "ffmpeg" || suite === "all";

  if (runUnit) {
    await runCommand(context, "cargo", base, { env });
  }
  if (runIgnored) {
    await runCommand(context, "cargo", [...base, "--", "--ignored", "--test-threads=1"], {
      env,
    });
  }
}

export async function runTestSuiteCommand(
  context: CommandContext,
  options: TestCommandOptions
): Promise<number> {
  const profile = validateStandaloneOptions({
    mode: options.mode,
    profile: options.profile,
    suite: options.suite,
  });

  if (options.mode === "system") {
    await runTestSuite(context, options.suite, null, process.env);
    return 0;
  }

  assertStandaloneProfile(profile);
  const env = await setupStandaloneEnv(context, profile);
  await runTestSuite(context, options.suite, profile, env);
  return 0;
}

function isProfileSupportedOnTarget(profile: FfmpegProfile): boolean {
  const target = getTargetTriple();
  return profile !== "lgpl-vt" || isMacOsTarget(target);
}

export async function runTestMatrixCommand(context: CommandContext): Promise<number> {
  const combos: { mode: BuildMode; profile?: FfmpegProfile; suite: TestSuite; label: string }[] = [
      { mode: "system", suite: "discovery", label: "system + discovery" },
      { mode: "system", suite: "all", label: "system + all" },
      ...FFMPEG_PROFILES.filter(isProfileSupportedOnTarget).map((profile) => ({
        mode: "standalone" as BuildMode,
        profile,
        suite: "all" as TestSuite,
        label: `standalone + ${profile} + all`,
      })),
    ];

  const comboLabels = combos.map((c) => c.label).join(", ");
  console.log("[matrix] Running", combos.length, "combinations:", comboLabels);

  let failed = 0;
  for (const combo of combos) {
    const { mode, profile, suite, label } = combo;
    console.log("[matrix] Running", label);
    try {
      const profileResolved = validateStandaloneOptions({
        mode,
        profile,
        suite,
      });
      if (mode === "system") {
        await runTestSuite(context, suite, null, process.env);
      } else {
        assertStandaloneProfile(profileResolved);
        const env = await setupStandaloneEnv(context, profileResolved);
        await runTestSuite(context, suite, profileResolved, env);
      }
      if (context.verbose) {
        console.log(`[matrix] ${label}: ok`);
      }
    } catch (err) {
      failed += 1;
      console.error(`[matrix] ${label}: FAILED`);
      console.error(err instanceof Error ? err.message : String(err));
    }
  }

  if (failed > 0) {
    console.error(`[matrix] ${String(failed)}/${String(combos.length)} combinations failed`);
    return 1;
  }
  if (context.verbose) {
    console.log(`[matrix] all ${String(combos.length)} combinations passed`);
  }
  return 0;
}
