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
    <Alert variant="destructive" className={cn("relative select-auto")}>
      <TriangleAlert className={cn("size-5")} />
      <Button
        size="icon"
        variant="ghost"
        onClick={() => getCompressionState().dismissError()}
        className={cn("absolute top-2 right-2 size-8")}
      >
        <XIcon />
      </Button>
      <AlertTitle>{error.type || "Error"}</AlertTitle>
      <AlertDescription className="">
        {error.message}
        {error.detail && error.detail !== error.message && (
          <details className={cn("mt-1")}>
            <summary className={cn("cursor-pointer text-sm")}>Cause</summary>
            <pre
              className={cn(
                "mt-1 max-h-32 overflow-auto text-xs wrap-anywhere",
                "whitespace-pre-wrap"
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
