import * as React from "react"
import { cn } from "cn"

import { fieldClassName } from "./input"

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "flex field-sizing-content min-h-16 w-full rounded-md px-3 py-2 text-base md:text-sm",
        fieldClassName,
        className
      )}
      {...props}
    />
  )
}

export { Textarea }
