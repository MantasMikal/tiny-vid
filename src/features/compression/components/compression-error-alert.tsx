import { TriangleAlert, XIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { getCompressionState } from "@/features/compression/store/compression-store";
import { cn } from "@/lib/utils";

interface CompressionErrorAlertProps {
  error: { type: string; message: string; detail?: string };
}

export function CompressionErrorAlert({ error }: CompressionErrorAlertProps) {
  return (
    <Alert variant="destructive" className={cn("relative bg-black pr-10")}>
      <TriangleAlert className={cn("size-5")} />
      <Button
        size="icon"
        variant="ghost"
        onClick={() => {
          getCompressionState().dismissError();
        }}
        className={cn("absolute top-2 right-2 size-8 text-current", "hover:bg-white/20")}
      >
        <XIcon className={cn("size-4")} />
      </Button>
      <AlertTitle>{error.type || "Error"}</AlertTitle>
      <AlertDescription>
        {error.message}
        {error.detail && error.detail !== error.message && (
          <details className={cn("mt-2")}>
            <summary className={cn("cursor-pointer text-sm opacity-80")}>Show details</summary>
            <pre
              className={cn(
                "mt-1 max-h-32 overflow-auto text-xs wrap-anywhere",
                "whitespace-pre-wrap opacity-90"
              )}
            >
              {error.detail}
            </pre>
          </details>
        )}
      </AlertDescription>
    </Alert>
  );
}
