// 印花税税源信息采集表导出：选申报类型 / 申报期限类型（默认 11 正常申报、00 按期申报）→ 选保存位置 → 生成。
// 采集表只汇总所选批次已确认的明细：批次 1 = 原申报清单；补充批次 = 只含补充的明细。
// 补充批次不会自动套用「21 纳税人自行补正」，申报类型由用户自己选。

import { DownloadSimple } from "@phosphor-icons/react";
import { save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import { commands, type DeclareType, type PeriodType } from "@/bindings";
import { AppButton } from "@/components/app-button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "@/components/ui/toast";

import { fenToYuan, getErrorMessage } from "./utils";

const DECLARE_TYPES: { value: DeclareType; label: string }[] = [
  { value: "11", label: "11 正常申报" },
  { value: "21", label: "21 纳税人自行补正" },
];

const PERIOD_TYPES: { value: PeriodType; label: string }[] = [
  { value: "00", label: "00 按期申报" },
  { value: "01", label: "01 按次申报" },
];

export function CollectionFormButton({
  filingFrom,
  filingTo,
  batchNo,
  disabled,
}: {
  filingFrom: string;
  filingTo: string;
  /** 导出哪个批次 */
  batchNo: number;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [declareType, setDeclareType] = useState<DeclareType>("11");
  const [periodType, setPeriodType] = useState<PeriodType>("00");
  const [busy, setBusy] = useState(false);

  async function exportForm() {
    // 文件名：{纳税所属期}_印花税税源信息采集表.xlsx（补充批次加「_补充」），由后端按所属期生成
    const name = await commands.stampTaxCollectionFormFilename(filingFrom, filingTo, batchNo);
    const path = await save({
      defaultPath: name,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!path) return;

    setBusy(true);
    try {
      const result = await commands.exportStampTaxCollectionForm({
        filingFrom,
        filingTo,
        batchNo,
        declareType,
        periodType,
        path,
      });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));
      toast.add({
        type: "success",
        title: "采集表已生成",
        description: `${result.data.rowCount} 行，计税金额 ${fenToYuan(result.data.totalTaxable)} 元，应纳税额 ${fenToYuan(result.data.totalTax)} 元`,
      });
      setOpen(false);
    } catch (error) {
      toast.add({ type: "error", title: "生成失败", description: getErrorMessage(error) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <AppButton type="button" size="sm" variant="glass" disabled={disabled} onClick={() => setOpen(true)}>
        <DownloadSimple data-icon="inline-start" />
        导出采集表
      </AppButton>

      <Dialog open={open} onOpenChange={(o) => !o && !busy && setOpen(false)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>导出印花税税源信息采集表</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-3">
            <p className="text-xs text-muted-foreground">
              {filingFrom === filingTo ? filingFrom : `${filingFrom}-${filingTo}`}
              {batchNo > 1 ? `，补充批次 ${batchNo}（只含补充明细）` : "，批次 1"}。
              按子目汇总计税金额后计算应纳税额。
            </p>
            <div className="flex flex-col gap-1">
              <Label className="text-muted-foreground">申报类型</Label>
              <Select
                items={DECLARE_TYPES}
                value={declareType}
                onValueChange={(v) => setDeclareType(v as DeclareType)}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {DECLARE_TYPES.map((o) => (
                    <SelectItem key={o.value} value={o.value}>
                      {o.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1">
              <Label className="text-muted-foreground">申报期限类型</Label>
              <Select
                items={PERIOD_TYPES}
                value={periodType}
                onValueChange={(v) => setPeriodType(v as PeriodType)}
              >
                <SelectTrigger className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PERIOD_TYPES.map((o) => (
                    <SelectItem key={o.value} value={o.value}>
                      {o.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <AppButton type="button" variant="glass" disabled={busy} onClick={() => setOpen(false)}>
              取消
            </AppButton>
            <AppButton type="button" variant="brand" disabled={busy} onClick={() => void exportForm()}>
              选择保存位置并生成
            </AppButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
