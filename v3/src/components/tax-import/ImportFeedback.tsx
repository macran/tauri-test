import type { ReactNode } from 'react'
import { WarningCircle } from '@phosphor-icons/react'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

/** 导入进行中的加载态卡片 */
export function ImportLoadingCard({ label, fileName }: { label: string; fileName: string }) {
  return (
    <div className="flex w-full max-w-xl flex-col items-center gap-5 rounded-2xl border border-border bg-card px-8 py-14">
      <div className="relative h-12 w-12">
        <div className="absolute inset-0 rounded-full border-4 border-muted" />
        <div className="absolute inset-0 animate-spin rounded-full border-4 border-t-primary" />
      </div>
      <div className="text-center">
        <p className="text-sm font-medium text-foreground">{label}</p>
        <p className="mt-1 max-w-xs truncate text-xs text-muted-foreground">{fileName}</p>
      </div>
    </div>
  )
}

/** 导入命令本身失败（文件读取、解析崩溃等系统级错误）时展示 */
export function ImportErrorAlert({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <Alert variant="destructive" className="w-full max-w-xl">
      <WarningCircle weight="fill" />
      <AlertTitle>导入失败</AlertTitle>
      <AlertDescription>{message}</AlertDescription>
      <Button variant="outline" size="sm" className="mt-3" onClick={onRetry}>
        重新选择文件
      </Button>
    </Alert>
  )
}

type PillTone = 'success' | 'error' | 'warning' | 'info'

const TONE_MAP: Record<PillTone, { cls: string; dot: string }> = {
  success: { cls: 'badge-matched', dot: 'var(--accent-green)' },
  error: { cls: 'badge-error', dot: 'var(--accent-red)' },
  warning: { cls: 'badge-pending', dot: 'var(--accent-amber)' },
  info: { cls: 'badge-parsing', dot: 'var(--blue-light)' },
}

/** 状态圆点徽标，复用全局 badge-* 主题 class（见 index.css），与仪表盘状态列保持同一视觉语言 */
export function StatusPill({ tone, children }: { tone: PillTone; children: ReactNode }) {
  const t = TONE_MAP[tone]
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[11px] font-semibold',
        t.cls
      )}
    >
      <span className="h-1.5 w-1.5 rounded-full" style={{ background: t.dot, boxShadow: `0 0 4px ${t.dot}` }} />
      {children}
    </span>
  )
}

const STAT_TONE_CLASS: Record<'default' | PillTone, string> = {
  default: 'text-foreground',
  success: 'text-[var(--accent-green)]',
  error: 'text-[var(--accent-red)]',
  warning: 'text-[var(--accent-amber)]',
  info: 'text-[var(--blue-light)]',
}

/** 结果页的数字统计格子 */
export function ImportStatCard({
  label,
  value,
  tone = 'default',
}: {
  label: string
  value: number
  tone?: 'default' | PillTone
}) {
  return (
    <div className="rounded-xl border border-border bg-muted/30 px-4 py-3">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className={cn('mt-1 text-2xl font-semibold tabular-nums', STAT_TONE_CLASS[tone])}>
        {value.toLocaleString()}
      </p>
    </div>
  )
}
