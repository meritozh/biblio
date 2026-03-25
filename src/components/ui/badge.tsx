import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

const badgeVariants = cva(
  "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2",
  {
    variants: {
      variant: {
        default:
          "bg-primary/10 text-primary",
        secondary:
          "bg-secondary text-secondary-foreground",
        destructive:
          "bg-destructive/10 text-destructive",
        outline: "border border-input text-foreground",
        blue: "bg-[#E8F0FE] text-[#5383EC] dark:bg-[#5383EC]/20 dark:text-[#5383EC]",
        purple: "bg-[#F3E8FC] text-[#9065B0] dark:bg-[#9065B0]/20 dark:text-[#9065B0]",
        pink: "bg-[#FDE8F0] text-[#E255A1] dark:bg-[#E255A1]/20 dark:text-[#E255A1]",
        green: "bg-[#DBEDDB] text-[#4DAB9A] dark:bg-[#4DAB9A]/20 dark:text-[#4DAB9A]",
        orange: "bg-[#FAE9D9] text-[#D9730D] dark:bg-[#D9730D]/20 dark:text-[#D9730D]",
        yellow: "bg-[#FBF3DB] text-[#DFAB01] dark:bg-[#DFAB01]/20 dark:text-[#DFAB01]",
        gray: "bg-[#EBECEA] text-[#787774] dark:bg-[#787774]/20 dark:text-[#9B9A97]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLDivElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return (
    <div className={cn(badgeVariants({ variant }), className)} {...props} />
  )
}

export { Badge, badgeVariants }