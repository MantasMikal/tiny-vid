import { cn } from "@/lib/utils";

type SpinnerProps = {
  className?: string;
};

export function Spinner(props: SpinnerProps) {
  const { className } = props;
  return (
    <div
      className={cn(
        "inline-block h-8 w-8 animate-spin rounded-full border-4 border-solid border-current align-[-0.125em] text-surface motion-reduce:animate-[spin_1.5s_linear_infinite] border-black dark:border-white !border-e-transparent", className
      )}
      role="status"
    >
      <span className="!absolute !-m-px !h-px !w-px !overflow-hidden !whitespace-nowrap !border-0 !p-0 ![clip:rect(0,0,0,0)]">
        Loading...
      </span>
    </div>
  );
}
