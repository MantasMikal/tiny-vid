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

const INTEGRATION_SMOKE_TARGETS = [
  "integration_runtime_commands",
  "integration_transcode_smoke",
  "integration_preview_smoke",
  "integration_lifecycle",
] as const;

const INTEGRATION_CONTRACT_TARGETS = ["integration_transcode_contract"] as const;

function cargoFeatureArgs(
  profile: FfmpegProfile | null,
  extraFeatures: readonly string[] = []
): string[] {
  const features = [...extraFeatures];
  if (profile === "lgpl-vt") {
    features.push("lgpl");
  }
  if (features.length === 0) {
    return [];
  }
  return ["--features", features.join(",")];
}

async function runUnitSuite(
  context: CommandContext,
  profile: FfmpegProfile | null,
  env: NodeJS.ProcessEnv
): Promise<void> {
  await runCommand(
    context,
    "cargo",
    ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib", ...cargoFeatureArgs(profile)],
    { env }
  );
}

async function runDiscoverySuite(context: CommandContext, env: NodeJS.ProcessEnv): Promise<void> {
  await runCommand(context, "cargo", [...DISCOVERY_TEST_ARGS], { env });
}

async function runIntegrationTargets(
  context: CommandContext,
  profile: FfmpegProfile | null,
  env: NodeJS.ProcessEnv,
  targets: readonly string[]
): Promise<void> {
  for (const target of targets) {
    await runCommand(
      context,
      "cargo",
      [
        "test",
        "--manifest-path",
        "src-tauri/Cargo.toml",
        "--test",
        target,
        ...cargoFeatureArgs(profile, ["integration-test-api"]),
        "--",
        "--test-threads=1",
      ],
      { env }
    );
  }
}

async function runTestSuite(
  context: CommandContext,
  suite: TestSuite,
  mode: BuildMode,
  profile: FfmpegProfile | null,
  env: NodeJS.ProcessEnv
): Promise<void> {
  if (suite === "unit") {
    await runUnitSuite(context, profile, env);
    return;
  }
  if (suite === "integration-smoke") {
    await runIntegrationTargets(context, profile, env, INTEGRATION_SMOKE_TARGETS);
    return;
  }
  if (suite === "integration-contract") {
    await runIntegrationTargets(context, profile, env, INTEGRATION_CONTRACT_TARGETS);
    return;
  }
  if (suite === "discovery") {
    await runDiscoverySuite(context, env);
    return;
  }

  if (mode === "system") {
    await runUnitSuite(context, profile, env);
    await runIntegrationTargets(context, profile, env, INTEGRATION_SMOKE_TARGETS);
    await runDiscoverySuite(context, env);
    return;
  }

  await runUnitSuite(context, profile, env);
  await runIntegrationTargets(context, profile, env, INTEGRATION_SMOKE_TARGETS);
  await runIntegrationTargets(context, profile, env, INTEGRATION_CONTRACT_TARGETS);
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
    await runTestSuite(context, options.suite, options.mode, null, process.env);
    return 0;
  }

  assertStandaloneProfile(profile);
  const env = await setupStandaloneEnv(context, profile);
  await runTestSuite(context, options.suite, options.mode, profile, env);
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
        await runTestSuite(context, suite, mode, null, process.env);
      } else {
        assertStandaloneProfile(profileResolved);
        const env = await setupStandaloneEnv(context, profileResolved);
        await runTestSuite(context, suite, mode, profileResolved, env);
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
