import * as SliderPrimitive from "@radix-ui/react-slider";
import * as React from "react";

import { cn } from "@/lib/utils";

interface SliderProps extends React.ComponentProps<typeof SliderPrimitive.Root> {
  /** When true, shows the current value inside the thumb */
  showValueOnThumb?: boolean;
  /** Format the value for the thumb label. Defaults to String(value) */
  formatThumbValue?: (value: number) => string;
}

function Slider({
  className,
  defaultValue,
  value,
  min = 0,
  max = 100,
  showValueOnThumb = false,
  formatThumbValue = (v) => String(v),
  ...props
}: SliderProps) {
  const _values = React.useMemo(
    () => (Array.isArray(value) ? value : Array.isArray(defaultValue) ? defaultValue : [min, max]),
    [value, defaultValue, min, max]
  );

  return (
    <SliderPrimitive.Root
      data-slot="slider"
      defaultValue={defaultValue}
      value={value}
      min={min}
      max={max}
      className={cn(
        `
          relative flex w-full touch-none items-center select-none
          data-disabled:opacity-50
          data-[orientation=vertical]:h-full data-[orientation=vertical]:min-h-44 data-[orientation=vertical]:w-auto
          data-[orientation=vertical]:flex-col
        `,
        className
      )}
      {...props}
    >
      <SliderPrimitive.Track
        data-slot="slider-track"
        className={cn(
          `
            relative grow overflow-hidden rounded-full bg-secondary
            data-[orientation=horizontal]:h-2 data-[orientation=horizontal]:w-full
            data-[orientation=vertical]:h-full data-[orientation=vertical]:w-1.5
          `
        )}
      >
        <SliderPrimitive.Range
          data-slot="slider-range"
          className={cn(
            `absolute bg-primary data-[orientation=horizontal]:h-full data-[orientation=vertical]:w-full`
          )}
        />
      </SliderPrimitive.Track>
      {Array.from({ length: _values.length }, (_, index) => (
        <SliderPrimitive.Thumb
          data-slot="slider-thumb"
          key={index}
          className={cn(
            `
              relative flex h-6 w-9 shrink-0 cursor-pointer items-center justify-center rounded-full border-2
              border-primary bg-background ring-offset-background transition-colors
              hover:bg-background
              focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:outline-none
              disabled:pointer-events-none disabled:opacity-50
            `
          )}
        >
          {showValueOnThumb && (
            <span
              className={cn(
                "text-[10px] leading-none font-medium select-none",
                "pointer-events-none overflow-hidden text-primary",
                "truncate text-center"
              )}
            >
              {formatThumbValue(_values[index])}
            </span>
          )}
        </SliderPrimitive.Thumb>
      ))}
    </SliderPrimitive.Root>
  );
}

export { Slider };
