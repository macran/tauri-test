// 认证清单导入 —— 数据导入 / 税务数据
// 页面结构见 ImportShell：顶部操作栏始终可用，可一次选多个文件，结果累积成记录列表。
import { ListChecks, XCircle } from '@phosphor-icons/react'
import { commands, type CertifiedImportSummary, type RowParseError } from '@/bindings'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { ImportStatCard } from '@/components/tax-import/ImportFeedback'
import { ImportShell } from '@/components/tax-import/ImportShell'
import { errorText, useImportQueue } from '@/components/tax-import/useImportQueue'

const hasAnyError = (s: CertifiedImportSummary) =>
  s.parseErrorCount > 0 || s.headerNotFoundCount > 0 || s.duplicateCount > 0

export default function CertifiedImportPage() {
  const queue = useImportQueue<CertifiedImportSummary>(async ({ filePath }) => {
    try {
      const result = await commands.importCertifiedExcel({ filePath })
      return result.status === 'ok'
        ? { ok: true, summary: result.data }
        : { ok: false, message: result.error || '导入失败' }
    } catch (err) {
      return { ok: false, message: errorText(err) }
    }
  })

  return (
    <ImportShell<CertifiedImportSummary>
      title="认证清单导入"
      description="导入增值税抵扣认证清单 Excel 文件，导入前请确保对应发票已入库"
      icon={<ListChecks className="mt-0.5 h-6 w-6 shrink-0 text-primary" weight="duotone" />}
      entries={queue.entries}
      onFiles={(files) => queue.enqueue(files, undefined)}
      onClear={queue.clearFinished}
      onRetry={(id) => queue.retry(id)}
      statusOf={(s) =>
        !s.committed
          ? { tone: 'error', text: '已回滚' }
          : hasAnyError(s)
            ? { tone: 'warning', text: '部分成功' }
            : { tone: 'success', text: '导入成功' }
      }
      summaryLine={(s) =>
        `所属期 ${s.taxPeriod} · 成功 ${s.inserted} · 解析失败 ${s.parseErrorCount} · 发票未入库 ${s.headerNotFoundCount} · 重复 ${s.duplicateCount}`
      }
      renderResult={(s) => <CertifiedImportResult summary={s} />}
    />
  )
}

function CertifiedImportResult({ summary }: { summary: CertifiedImportSummary }) {
  return (
    <div className="flex flex-col gap-5">
      <div>
        <p className="mb-3 text-xs text-muted-foreground">税款所属期 {summary.taxPeriod}</p>
        <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
          <ImportStatCard label="成功导入" value={summary.inserted} />
          <ImportStatCard
            label="解析失败"
            value={summary.parseErrorCount}
            tone={summary.parseErrorCount > 0 ? 'error' : 'default'}
          />
          <ImportStatCard
            label="发票未入库"
            value={summary.headerNotFoundCount}
            tone={summary.headerNotFoundCount > 0 ? 'warning' : 'default'}
          />
          <ImportStatCard label="重复跳过" value={summary.duplicateCount} />
        </div>

        {summary.rollbackReason && (
          <Alert variant="destructive" className="mt-4">
            <XCircle weight="fill" />
            <AlertTitle>回滚原因</AlertTitle>
            <AlertDescription>{summary.rollbackReason}</AlertDescription>
          </Alert>
        )}
      </div>

      {summary.parseErrorCount > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-red)]">
              解析失败明细（共 {summary.parseErrorCount} 条）
            </CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="text-center">行</TableHead>
                  <TableHead>列</TableHead>
                  <TableHead>错误信息</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.parseErrors.map((err: RowParseError, i: number) => (
                  <TableRow key={i}>
                    <TableCell className="text-center text-muted-foreground">{err.row}</TableCell>
                    <TableCell className="font-mono text-muted-foreground">{err.col}</TableCell>
                    <TableCell className="text-[var(--accent-red)]">{err.message}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}

      {summary.headerNotFoundCount > 0 && (
        <Card>
          <CardHeader>
            <CardTitle className="text-[var(--accent-amber)]">
              发票未入库（共 {summary.headerNotFoundCount} 条）
            </CardTitle>
          </CardHeader>
          <CardContent className="px-0">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>发票唯一标识（inv_unq_key）</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {summary.headerNotFoundKeys.map((key, i) => (
                  <TableRow key={i}>
                    <TableCell className="font-mono">{key}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
