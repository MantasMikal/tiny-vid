import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/** Safely extract a display string from an unknown caught value. */
export function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
