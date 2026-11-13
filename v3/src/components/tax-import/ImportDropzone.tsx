import { useCallback, useRef } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { CloudArrowUp } from '@phosphor-icons/react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useFileDrop } from '@/lib/useFileDrop'

interface ImportDropzoneProps {
  /** 拖拽区默认提示文案，如“拖拽全量发票文件到此处” */
  label: string
  /** 选择按钮文案 */
  pickButtonLabel: string
  /** 文件就位后的回调，由调用方决定如何处理（通常是切换到 loading 状态并调用后端命令） */
  onFileSelected: (filePath: string, fileName: string) => void
  /** 打开系统文件选择对话框失败时触发（例如 Tauri capability 权限未授予），用于让调用方展示可见的错误提示 */
  onError?: (message: string) => void
  extensions?: string[]
  filterName?: string
  disabled?: boolean
}

export function ImportDropzone({
  label,
  pickButtonLabel,
  onFileSelected,
  onError,
  extensions = ['xlsx', 'xls'],
  filterName = 'Excel 文件',
  disabled = false,
}: ImportDropzoneProps) {
  const dropRef = useRef<HTMLDivElement>(null)

  const handlePickFile = useCallback(async () => {
    if (disabled) return
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: filterName, extensions }],
      })
      if (!selected) return
      const filePath = selected
      const fileName = filePath.split(/[\\/]/).pop() ?? filePath
      onFileSelected(filePath, fileName)
    } catch (err: unknown) {
      onError?.(typeof err === 'string' ? err : ((err as Error)?.message ?? '无法打开文件选择对话框'))
    }
  }, [disabled, extensions, filterName, onError, onFileSelected])

  // 拖入：Tauri 事件给出真实路径（HTML5 drop 的 File 没有路径）
  const isDragging = useFileDrop(dropRef, onFileSelected, { disabled, extensions })

  return (
    <div
      ref={dropRef}
      onClick={handlePickFile}
      className={cn(
        'flex w-full max-w-xl flex-col items-center justify-center gap-4 rounded-2xl border-2 border-dashed px-8 py-14 transition-all duration-200',
        disabled ? 'cursor-not-allowed opacity-60' : 'cursor-pointer',
        isDragging
          ? 'scale-[1.01] border-primary bg-primary/5'
          : 'border-border bg-card hover:border-primary/40 hover:bg-muted/40'
      )}
    >
      <div className={cn('rounded-full p-4 transition-colors', isDragging ? 'bg-primary/10' : 'bg-muted')}>
        <CloudArrowUp
          className={cn('h-8 w-8', isDragging ? 'text-primary' : 'text-muted-foreground')}
          weight={isDragging ? 'fill' : 'regular'}
        />
      </div>

      <div className="text-center">
        <p className={cn('text-base font-medium', isDragging ? 'text-primary' : 'text-foreground')}>
          {isDragging ? '松开以开始导入' : label}
        </p>
        <p className="mt-1 text-sm text-muted-foreground">或点击下方按钮选择文件</p>
      </div>

      <Button
        type="button"
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation()
          handlePickFile()
        }}
      >
        {pickButtonLabel}
      </Button>

      <p className="text-xs text-muted-foreground">{extensions.map((ext) => `.${ext}`).join(' · ')}</p>
    </div>
  )
}
