import { useState } from "react";

import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

export interface ClampedNumberInputProps {
  value: number;
  min: number;
  max: number;
  disabled?: boolean;
  onChange: (value: number) => void;
  /** Optional callback when editing ends (blur) */
  onBlur?: () => void;
  className?: string;
}

/**
 * A number input that clamps values to min/max and supports free-form typing
 * while focused. Shows the formatted value when not editing; allows typing
 * (including empty/invalid) while focused, then validates and clamps on blur.
 */
export function ClampedNumberInput({
  value,
  min,
  max,
  disabled,
  onChange,
  onBlur: onBlurProp,
  className,
}: ClampedNumberInputProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [inputValue, setInputValue] = useState("");

  const displayedValue = isEditing ? inputValue : String(value);

  const clamp = (n: number) => Math.min(max, Math.max(min, n));

  return (
    <Input
      disabled={disabled}
      type="number"
      min={min}
      max={max}
      value={displayedValue}
      className={cn(className)}
      onFocus={() => {
        setIsEditing(true);
        setInputValue(String(value));
      }}
      onBlur={() => {
        setIsEditing(false);
        if (!inputValue.trim()) {
          setInputValue(String(value));
          onBlurProp?.();
          return;
        }
        const parsed = Number.parseFloat(inputValue);
        if (!Number.isFinite(parsed)) {
          setInputValue(String(value));
          onBlurProp?.();
          return;
        }
        const clamped = clamp(parsed);
        if (clamped !== value) {
          onChange(clamped);
        }
        setInputValue(String(clamped));
        onBlurProp?.();
      }}
      onChange={(e) => {
        const nextValue = e.target.value;
        setInputValue(nextValue);
        if (!nextValue.trim()) return;
        const parsed = Number.parseFloat(nextValue);
        if (!Number.isFinite(parsed)) return;
        const clamped = clamp(parsed);
        onChange(clamped);
      }}
    />
  );
}
