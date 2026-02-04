import { motion } from "motion/react";

import { cn } from "@/lib/utils";

interface FadeInProps extends React.ComponentPropsWithoutRef<
  typeof motion.div
> {
  duration?: number;
}

export function FadeIn({
  className,
  children,
  duration = 0.2,
  ...props
}: FadeInProps) {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration }}
      className={cn(className)}
      {...props}
    >
      {children}
    </motion.div>
  );
}
