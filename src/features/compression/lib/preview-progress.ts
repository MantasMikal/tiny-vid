const STEP_LABELS: Record<string, string> = {
  preview_extract: "Extracting…",
  preview_transcode: "Generating preview…",
  preview_estimate: "Estimating size…",
  generating_preview: "Generating preview…",
};

export function getProgressStepLabel(step: string | null): string {
  return (step && STEP_LABELS[step]) ?? "Processing…";
}
