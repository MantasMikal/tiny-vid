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
  if (error instanceof Error) {
    return { type, message: error.message, detail: error.message };
  }
  const message = String(error);
  return { type, message, detail: message };
}

function isAbortError(error: unknown): boolean {
  return (
    error instanceof Error &&
    (error.name === "AbortError" || error.message === "Aborted")
  );
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
