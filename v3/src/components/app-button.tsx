import type { ComponentProps } from "react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

// 项目自己的按钮风格，包在 shadcn Button 外面：ui/button.tsx 保持原样，
// 以后 `shadcn add` 覆盖它也不会丢这些变体。
//   glass  次要操作（取消/重置/解锁）
//   brand  重点操作（保存/查询/确认），主题色渐变
//   danger 危险操作（锁定）
// 其余 variant（outline/ghost/…）原样透传给 Button。
const APP_VARIANTS = {
  glass: "btn-glass",
  brand: "btn-brand",
  danger: "btn-danger",
} as const

type AppVariant = keyof typeof APP_VARIANTS
type ButtonProps = ComponentProps<typeof Button>

export function AppButton({
  variant,
  className,
  ...props
}: Omit<ButtonProps, "variant"> & { variant?: AppVariant | ButtonProps["variant"] }) {
  if (variant && variant in APP_VARIANTS) {
    return (
      <Button
        variant="ghost"
        className={cn(APP_VARIANTS[variant as AppVariant], className)}
        {...props}
      />
    )
  }
  return <Button variant={variant as ButtonProps["variant"]} className={className} {...props} />
}
