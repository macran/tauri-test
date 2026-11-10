import { Loader2, Plus, Upload } from "lucide-react";
import { useEffect, useState } from "react";

import {
  commands,
  type StampTaxCategoryOption,
  type TaxMatchRuleRecord,
} from "@/bindings";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { AppButton } from "@/components/app-button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { toast } from "@/components/ui/toast";

import RuleExcelImport from "@/components/RuleExcelImport";
import { ExportButton } from "@/features/query/ExportButton";
import { TaxRuleFormDialog } from "./components/TaxRuleFormDialog";
import { TaxRulesTable } from "./components/TaxRulesTable";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "未知错误";
}

export default function TaxRulesPage() {
  const [rules, setRules] = useState<TaxMatchRuleRecord[]>([]);

  const [stampCategories, setStampCategories] = useState<
    StampTaxCategoryOption[]
  >([]);

  const [isLoading, setIsLoading] = useState(false);

  const [formOpen, setFormOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [ruleToDisable, setRuleToDisable] = useState<TaxMatchRuleRecord | null>(
    null
  );
  const [isDisabling, setIsDisabling] = useState(false);

  async function loadRules() {
    setIsLoading(true);

    try {
      const result = await commands.listTaxMatchRules(true);

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      setRules(result.data);
    } catch (error) {
      toast.add({
        type: "error",
        title: "读取规则失败",
        description: getErrorMessage(error),
      });
    } finally {
      setIsLoading(false);
    }
  }

  async function loadStampCategories() {
    try {
      const result = await commands.listStampTaxCategories();

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      setStampCategories(result.data);
    } catch (error) {
      toast.add({
        type: "error",
        title: "读取印花税税目失败",
        description: getErrorMessage(error),
      });
    }
  }

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once on mount
  useEffect(() => {
    void loadRules();
    void loadStampCategories();
  }, []);

  async function handleDisable() {
    if (!ruleToDisable) {
      return;
    }

    setIsDisabling(true);

    try {
      const result = await commands.disableTaxMatchRule(ruleToDisable.id);

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      setRuleToDisable(null);
      toast.add({ type: "success", title: "规则已禁用" });
      await loadRules();
    } catch (error) {
      toast.add({
        type: "error",
        title: "禁用规则失败",
        description: getErrorMessage(error),
      });
    } finally {
      setIsDisabling(false);
    }
  }

  return (
    <>
      <div className="flex h-full min-h-0 min-w-0 flex-col">
        <div className="flex h-12 shrink-0 items-center gap-3 border-b border-border px-4">
          <h2 className="text-sm font-semibold text-foreground">规则管理</h2>

          <div className="ml-auto flex items-center gap-2">
            <AppButton type="button" variant="glass" onClick={() => setImportOpen(true)}>
              <Upload data-icon="inline-start" />
              导入规则
            </AppButton>
            <AppButton type="button" variant="brand" onClick={() => setFormOpen(true)}>
              <Plus data-icon="inline-start" />
              添加规则
            </AppButton>
          </div>
        </div>

        <div className="flex min-h-0 flex-1 flex-col p-3">
          <Card className="min-h-0 flex-1">
            <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
              <TaxRulesTable
                data={rules}
                isLoading={isLoading}
                onDisable={setRuleToDisable}
                toolbarSlot={
                  <>
                    <ExportButton
                      label="导出启用规则"
                      defaultName="规则_启用"
                      disabled={!rules.some((r) => r.enabled)}
                      run={(path) => commands.exportTaxRules(false, path)}
                    />
                    <ExportButton
                      label="导出全部规则"
                      defaultName="规则_全部"
                      disabled={rules.length === 0}
                      run={(path) => commands.exportTaxRules(true, path)}
                    />
                  </>
                }
              />
            </CardContent>
          </Card>
        </div>
      </div>

      <TaxRuleFormDialog
        open={formOpen}
        stampCategories={stampCategories}
        onOpenChange={setFormOpen}
        onSaved={async () => {
          setFormOpen(false);
          await loadRules();
        }}
      />

      <Dialog open={importOpen} onOpenChange={setImportOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>导入规则</DialogTitle>
          </DialogHeader>
          <RuleExcelImport onImported={() => void loadRules()} />
        </DialogContent>
      </Dialog>

      <AlertDialog
        open={ruleToDisable !== null}
        onOpenChange={(open) => {
          if (!open) {
            setRuleToDisable(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认禁用规则？</AlertDialogTitle>

            <AlertDialogDescription>
              禁用后，该规则不会再参与匹配，并且不能重新启用。
              如果以后需要使用，只能重新添加。
            </AlertDialogDescription>
          </AlertDialogHeader>

          <AlertDialogFooter>
            <AlertDialogCancel disabled={isDisabling}>取消</AlertDialogCancel>

            <AlertDialogAction
              disabled={isDisabling}
              onClick={(event) => {
                event.preventDefault();
                void handleDisable();
              }}
            >
              {isDisabling && <Loader2 className="mr-2 size-4 animate-spin" />}
              确认禁用
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
