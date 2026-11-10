import { cn } from '@/lib/utils'

function Skeleton({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      data-slot='skeleton'
      // 骨架屏要的是「无色」的灰：语义色 token（含 --accent/--muted）在本主题里
      // 都带有青色调性，用在这里会被误认成品牌色而不是占位符，所以这里固定用
      // 零彩度的中性灰阶，不跟随主题的品牌色变化。
      className={cn('animate-pulse rounded-md bg-zinc-200 dark:bg-zinc-100/10', className)}
      {...props}
    />
  )
}

export { Skeleton }
