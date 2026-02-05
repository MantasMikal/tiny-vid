export interface ResultError {
  type: string;
  message: string;
  detail?: string;
}

export type TryResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: ResultError; aborted: false }
  | { ok: false; aborted: true };

function toResultError(error: unknown, type: string): ResultError {
  const raw = error instanceof Error ? error.message : String(error);
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (
      parsed &&
      typeof parsed === "object" &&
      typeof (parsed as { summary?: unknown }).summary === "string" &&
      typeof (parsed as { detail?: unknown }).detail === "string"
    ) {
      const { summary, detail } = parsed as { summary: string; detail: string };
      return { type, message: summary, detail };
    }
  } catch {
    /* not JSON */
  }
  return { type, message: raw, detail: raw };
}

function isAbortError(error: unknown): boolean {
  return error instanceof Error && (error.name === "AbortError" || error.message === "Aborted");
}

export async function tryCatch<T>(
  fn: () => Promise<T>,
  errorType: string,
  options?: { onFinally?: () => void }
): Promise<TryResult<T>> {
  try {
    const value = await fn();
    return { ok: true, value };
  } catch (error) {
    console.log(`[${errorType}]`, error);
    if (isAbortError(error)) {
      return { ok: false, aborted: true };
    }
    return {
      ok: false,
      error: toResultError(error, errorType),
      aborted: false,
    };
  } finally {
    options?.onFinally?.();
  }
}
