import { motion } from "motion/react";

import { cn } from "@/lib/utils";

interface FadeInProps extends React.ComponentPropsWithoutRef<typeof motion.div> {
  duration?: number;
  delay?: number;
}

export function FadeIn({ className, children, duration = 0.2, delay = 0, ...props }: FadeInProps) {
  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration, delay }}
      className={cn(className)}
      {...props}
    >
      {children}
    </motion.div>
  );
}
