import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { CheckIcon } from "lucide-react";
import * as React from "react";

import { cn } from "@/lib/utils";

function Checkbox({
  className,
  ...props
}: React.ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        `
          peer size-5 shrink-0 rounded-sm border border-primary shadow-sm
          transition-shadow outline-none
          focus-visible:ring-1 focus-visible:ring-ring
          focus-visible:outline-none
          disabled:cursor-not-allowed disabled:opacity-50
          aria-invalid:border-destructive aria-invalid:ring-destructive/20
          data-[state=checked]:bg-primary
          data-[state=checked]:text-primary-foreground
          dark:bg-input/30
          dark:aria-invalid:ring-destructive/40
          dark:data-[state=checked]:bg-primary
        `,
        className
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator
        data-slot="checkbox-indicator"
        className={cn("grid place-content-center text-current transition-none")}
      >
        <CheckIcon className={cn("relative size-4")} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

export { Checkbox };
