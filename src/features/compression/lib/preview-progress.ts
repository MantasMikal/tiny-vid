const STEP_LABELS: Record<string, string> = {
  generating_preview: "Generating preview…",
  transcode: "Compressing…",
  preview_extract: "Extracting segments…",
  preview_transcode: "Compressing…",
  preview_estimate: "Estimating size…",
};

export function getProgressStepLabel(step: string | null): string {
  return (step && STEP_LABELS[step]) ?? "Processing…";
}
