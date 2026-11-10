import * as React from "react"
import { Input as InputPrimitive } from "@base-ui/react/input"
import { cn } from "cn"

/**
 * 与 Win11DatePicker 的输入框同一套状态（见 win11-date-picker.tsx）：
 *   默认   淡灰边框（--field-border）+ 浅填充
 *   hover  边框加深（--field-border-hover）
 *   聚焦   上/左/右边框粗细不变、换成主题色；下沿多一道 2px 主题色线（inset 阴影）。不再有外圈（ring）。
 *   出错   整套换成 destructive
 * 主题色：亮色用 --blue-primary，暗色用 --accent-amber（与日期组件的 --w11-accent 一致）。
 * 注意不能用 --w11-* 变量：它们只在 win11-date-picker 容器内定义，这里取不到。
 */
export const fieldClassName = cn(
  "[--field-accent:var(--blue-primary)] dark:[--field-accent:var(--accent-amber)]",
  "border border-field-border bg-field-bg",
  "shadow-[inset_0_1px_0_rgb(0_0_0/0.02)]",
  "outline-none transition-[border-color,box-shadow] duration-150 motion-reduce:transition-none",
  "placeholder:text-muted-foreground",
  "hover:border-field-border-hover",
  "focus:border-[var(--field-accent)]",
  "focus:shadow-[inset_0_-2px_0_var(--field-accent)]",
  "disabled:cursor-not-allowed disabled:bg-muted/50 disabled:opacity-60 disabled:hover:border-field-border",
  "aria-invalid:border-destructive aria-invalid:bg-destructive/[0.03] aria-invalid:hover:border-destructive",
  "aria-invalid:focus:border-destructive aria-invalid:focus:shadow-[inset_0_-2px_0_var(--destructive)]",
)

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(
        "h-(--control-h) w-full min-w-0 rounded-md px-3 py-1 text-base md:text-sm file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-sm file:font-medium file:text-foreground disabled:pointer-events-none",
        fieldClassName,
        className
      )}
      {...props}
    />
  )
}

export { Input }
