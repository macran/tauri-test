import { useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  FileSpreadsheet,
  Info,
  Loader2,
  Upload,
  X,
} from "lucide-react";

import { commands, type RuleImportSummary } from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "./ui/alert";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "./ui/card";
import { Separator } from "./ui/separator";

function getFileName(filePath: string): string {
  return filePath.split(/[\\/]/).pop() ?? filePath;
}

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  if (error && typeof error === "object") {
    try {
      return JSON.stringify(error);
    } catch {
      return "发生未知错误";
    }
  }

  return "发生未知错误";
}

function formatImportError(error: {
  excelRow: number;
  message: string;
}): string {
  return `第 ${error.excelRow} 行：${error.message}`;
}

export default function RuleExcelImport({ onImported }: { onImported?: () => void }) {
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [summary, setSummary] = useState<RuleImportSummary | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const hasImportErrors = Boolean(
    summary && summary.invalidRows > 0,
  );

  async function handleChooseFile() {
    if (isImporting) {
      return;
    }

    setErrorMessage(null);
    setSummary(null);

    try {
      const selected = await open({
        multiple: false,
        directory: false,
        title: "选择税务规则 Excel 文件",
        filters: [
          {
            name: "Excel 文件",
            extensions: ["xlsx", "xlsm", "xls"],
          },
        ],
      });

      if (!selected) {
        return;
      }

      const filePath = Array.isArray(selected)
        ? selected[0]
        : selected;

      if (!filePath) {
        return;
      }

      setSelectedFile(filePath);
    } catch (error) {
      setErrorMessage(`选择文件失败：${getErrorMessage(error)}`);
    }
  }

  function handleClearFile() {
    if (isImporting) {
      return;
    }

    setSelectedFile(null);
    setSummary(null);
    setErrorMessage(null);
  }

  async function handleImport() {
  if (!selectedFile || isImporting) {
    return;
  }

  setIsImporting(true);
  setSummary(null);
  setErrorMessage(null);

  try {
    const result = await commands.importTaxRules(selectedFile);

    console.log("[rules] importTaxRules 返回", result);

    if (result.status === "ok") {
      console.log("[rules] 导入摘要", result.data);

      setSummary(result.data);
      if (result.data.inserted > 0) onImported?.();
      return;
    }

    setErrorMessage(`导入失败：${getErrorMessage(result.error)}`);
  } catch (error) {
    console.error("[rules] importTaxRules 调用异常", error);
    setErrorMessage(`导入失败：${getErrorMessage(error)}`);
  } finally {
    setIsImporting(false);
  }
}
  return (
    <Card className="w-full">
      <CardHeader className="space-y-3">
        <div className="flex items-start gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <FileSpreadsheet
              aria-hidden="true"
              className="size-5"
            />
          </div>

          <div className="min-w-0">
            <CardTitle>导入税务规则</CardTitle>
            <CardDescription className="mt-1.5 leading-6">
              Excel 数据从 Sheet1 第 3 行开始读取。
            </CardDescription>
          </div>
        </div>

        <Alert>
          <Info
            aria-hidden="true"
            className="size-4"
          />
          <AlertTitle>导入规则说明</AlertTitle>
          <AlertDescription>
            <ul className="mt-1 list-disc space-y-1 pl-4">
              <li>第 1、2 行不会导入数据库。</li>
              <li>E 列销售方名称仅用于 Excel 人工核对。</li>
              <li>存在错误行时，本批次不会写入数据库。</li>
              <li>新导入的规则默认启用。</li>
            </ul>
          </AlertDescription>
        </Alert>
      </CardHeader>

      <CardContent className="space-y-5">
        <section
          aria-labelledby="rule-file-section-title"
          className="space-y-3"
        >
          <div className="flex items-center justify-between gap-3">
            <div>
              <h2
                id="rule-file-section-title"
                className="text-sm font-medium"
              >
                规则文件
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">
                支持 `.xlsx`、`.xlsm` 和 `.xls` 文件
              </p>
            </div>

            {selectedFile ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleClearFile}
                disabled={isImporting}
                aria-label="清除已选择的文件"
              >
                <X
                  aria-hidden="true"
                  className="mr-1.5 size-4"
                />
                清除
              </Button>
            ) : null}
          </div>

          <div
            className={cn(
              "rounded-xl border border-dashed p-5 transition-colors",
              selectedFile
                ? "border-primary/40 bg-primary/[0.03]"
                : "border-muted-foreground/25 bg-muted/20",
            )}
          >
            {selectedFile ? (
              <div className="flex items-center gap-3">
                <div className="flex size-11 shrink-0 items-center justify-center rounded-lg bg-emerald-500/10 text-emerald-600">
                  <FileSpreadsheet
                    aria-hidden="true"
                    className="size-5"
                  />
                </div>

                <div className="min-w-0 flex-1">
                  <p
                    className="truncate text-sm font-medium"
                    title={selectedFile}
                  >
                    {getFileName(selectedFile)}
                  </p>
                  <p
                    className="mt-1 truncate text-xs text-muted-foreground"
                    title={selectedFile}
                  >
                    {selectedFile}
                  </p>
                </div>

                <Badge
                  variant="outline"
                  className="hidden shrink-0 sm:inline-flex"
                >
                  已选择
                </Badge>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center gap-3 py-5 text-center">
                <div className="flex size-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
                  <Upload
                    aria-hidden="true"
                    className="size-5"
                  />
                </div>

                <div>
                  <p className="text-sm font-medium">
                    尚未选择规则文件
                  </p>
                  <p className="mt-1 text-sm text-muted-foreground">
                    请选择包含规则数据的 Excel 文件
                  </p>
                </div>

                <Button
                  type="button"
                  variant="outline"
                  onClick={handleChooseFile}
                  disabled={isImporting}
                >
                  <FileSpreadsheet
                    aria-hidden="true"
                    className="mr-2 size-4"
                  />
                  选择 Excel 文件
                </Button>
              </div>
            )}
          </div>

          {selectedFile ? (
            <div className="flex flex-wrap items-center gap-2">
              <Button
                type="button"
                variant="outline"
                onClick={handleChooseFile}
                disabled={isImporting}
              >
                更换文件
              </Button>

              <Button
                type="button"
                onClick={handleImport}
                disabled={isImporting}
                className="min-w-32"
              >
                {isImporting ? (
                  <>
                    <Loader2
                      aria-hidden="true"
                      className="mr-2 size-4 animate-spin"
                    />
                    正在导入…
                  </>
                ) : (
                  <>
                    <Upload
                      aria-hidden="true"
                      className="mr-2 size-4"
                    />
                    导入规则
                  </>
                )}
              </Button>
            </div>
          ) : null}
        </section>

        {errorMessage ? (
          <>
            <Separator />

            <Alert
              variant="destructive"
              role="alert"
            >
              <AlertCircle
                aria-hidden="true"
                className="size-4"
              />
              <AlertTitle>操作失败</AlertTitle>
              <AlertDescription className="break-words">
                {errorMessage}
              </AlertDescription>
            </Alert>
          </>
        ) : null}

        {summary? (
          <>
            <Separator />

            <section
              aria-labelledby="import-result-title"
              aria-live="polite"
              className="space-y-4"
            >
              <div className="flex items-center gap-2">
                {hasImportErrors ? (
                  <AlertCircle
                    aria-hidden="true"
                    className="size-5 text-destructive"
                  />
                ) : (
                  <CheckCircle2
                    aria-hidden="true"
                    className="size-5 text-emerald-600"
                  />
                )}

                <div>
                  <h2
                    id="import-result-title"
                    className="text-sm font-medium"
                  >
                    {hasImportErrors
                      ? "导入校验未通过"
                      : "导入完成"}
                  </h2>

                  <p className="mt-1 text-sm text-muted-foreground">
                    {hasImportErrors
                      ? "请修正 Excel 中的错误行后重新导入。"
                      : "规则已完成校验并写入数据库。"}
                  </p>
                </div>
              </div>

              <div className="grid gap-3 sm:grid-cols-4">
                <ResultItem
                  label="读取行数"
                  value={summary?.totalRows}
                />
                <ResultItem
                  label="新增规则"
                  value={summary?.inserted}
                  valueClassName="text-emerald-600"
                />
                <ResultItem
                  label="重复跳过"
                  value={summary?.duplicates}
                  valueClassName="text-amber-600"
                />
                <ResultItem
                  label="错误行数"
                  value={summary?.invalidRows}
                  valueClassName={
                    summary?.invalidRows > 0
                      ? "text-destructive"
                      : undefined
                  }
                />
              </div>

              {hasImportErrors && summary.errors.length > 0 ? (
                <Alert variant="destructive">
                  <AlertCircle
                    aria-hidden="true"
                    className="size-4"
                  />
                  <AlertTitle>
                    请处理以下 Excel 行
                  </AlertTitle>
                  <AlertDescription>
                    <div className="mt-2 max-h-64 overflow-y-auto rounded-md border border-destructive/20 bg-background/60 p-3">
                      <ul className="space-y-2 text-sm">
                        {summary.errors.map((item) => (
                          <li
                            key={`${item.excelRow}-${item.message}`}
                            className="break-words"
                          >
                            {formatImportError(item)}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </AlertDescription>
                </Alert>
              ) : null}

              {!hasImportErrors ? (
                <Alert>
                  <CheckCircle2
                    aria-hidden="true"
                    className="size-4 text-emerald-600"
                  />
                  <AlertTitle>规则已启用</AlertTitle>
                  <AlertDescription>
                    本批次新增规则默认启用；与已有启用规则内容一致的行已自动跳过。
                  </AlertDescription>
                </Alert>
              ) : null}
            </section>
          </>
        ) : null}
      </CardContent>
    </Card>
  );
}

function ResultItem({
  label,
  value,
  valueClassName,
}: {
  label: string;
  value: number;
  valueClassName?: string;
}) {
  return (
    <div className="rounded-lg border bg-muted/20 px-4 py-3">
      <p className="text-xs text-muted-foreground">
        {label}
      </p>
      <p
        className={cn(
          "mt-1 text-2xl font-semibold tabular-nums",
          valueClassName,
        )}
      >
        {value}
      </p>
    </div>
  );
}