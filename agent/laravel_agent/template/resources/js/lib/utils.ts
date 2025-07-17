import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

/**
 * Combines class names using `clsx` then merges Tailwind classes with `tailwind-merge`.
 *
 * This helper replicates the `cn` utility expected by all shadcn/ui components.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
} 