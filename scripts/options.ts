import { Option } from "commander";

import { FFMPEG_PROFILE_INPUTS } from "./ffmpeg-profile.ts";
import { BUILD_MODES, BUILD_PLATFORMS, TEST_SUITES } from "./standalone.ts";

export function dryRunOption(): Option {
  return new Option("--dry-run", "Print commands and environment overrides without executing");
}

export function verboseOption(): Option {
  return new Option("--verbose", "Print extra decision and command output");
}

export function modeOption(): Option {
  return new Option("--mode <mode>").choices(BUILD_MODES).default("system");
}

export function profileOption(required = false): Option {
  const option = new Option("--profile <profile>").choices(FFMPEG_PROFILE_INPUTS);
  if (required) {
    option.makeOptionMandatory();
  }
  return option;
}

export function platformOption(): Option {
  return new Option("--platform <platform>").choices(BUILD_PLATFORMS).default("auto");
}

export function suiteOption(): Option {
  return new Option("--suite <suite>").choices(TEST_SUITES).default("all");
}
