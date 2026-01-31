import { cn } from "@/lib/utils";

export function WindowFrame({ children }: { children: React.ReactNode }) {
  return (
    <div
      className={cn(
        `
          flex grow flex-col select-none
          md:max-h-screen
        `,
        "transition-colors duration-300 ease-in-out"
      )}
    >
      {children}
    </div>
  );
}
