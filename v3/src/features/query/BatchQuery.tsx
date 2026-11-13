// 批量查询：粘贴 Excel 一列值，按“在清单中 / 不在清单中”精确匹配（不是模糊匹配）。
// 作为查询表单的补充，页面只需持有 BatchSet 状态并在提交时放进 filter.batch。

import { useState } from "react";
import { ListBullets } from "@phosphor-icons/react";
import type { BatchFilter, BatchSet } from "@/bindings";
import { AppButton } from "@/components/app-button";
import {
  Drawer,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";

export type BatchKey = keyof BatchSet;

export const BATCH_LABELS: Record<BatchKey, string> = {
  invoiceNumber: "发票号码",
  sellerName: "销方名称",
  sellerTaxNo: "销方税号",
  goodsName: "货物名称",
  taxCode: "税收分类编码",
};

/** 编辑态：文本框原文 + 是否“不包含”。原文保留，重新打开时不丢。 */
export type BatchDraft = Partial<Record<BatchKey, { text: string; exclude: boolean }>>;

/** 货物名称常带空格（如「A4 打印纸」「iPhone 15」），按空格切会把一个名称拆成两个，所以只有它不把空格当分隔符。
 *  其余字段（发票号码、销方名称、销方税号、税收分类编码）的值本身不含空格，空格 / 制表符 / 全角空格都算分隔符。 */
const NO_SPACE_SPLIT: BatchKey[] = ["goodsName"];

/** 按换行、中英文逗号切分（非货物名称再加空白符），每项去掉首尾空白，去空去重。 */
export function parseBatchText(text: string, key: BatchKey): string[] {
  const sep = NO_SPACE_SPLIT.includes(key) ? /[\r\n,，]+/ : /[\s,，]+/;
  return [...new Set(text.split(sep).map((v) => v.trim()).filter(Boolean))];
}

/** 草稿 → 提交给后端的 filter.batch；没有任何有效清单返回 null。 */
export function toBatchSet(draft: BatchDraft): BatchSet | null {
  const out: Record<string, BatchFilter | null> = {};
  let any = false;
  for (const k of Object.keys(BATCH_LABELS) as BatchKey[]) {
    const values = parseBatchText(draft[k]?.text ?? "", k);
    out[k] = values.length ? { values, exclude: draft[k]!.exclude } : null;
    any ||= values.length > 0;
  }
  return any ? (out as unknown as BatchSet) : null;
}

export function batchCount(draft: BatchDraft): number {
  return (Object.keys(BATCH_LABELS) as BatchKey[]).filter(
    (k) => parseBatchText(draft[k]?.text ?? "", k).length > 0,
  ).length;
}

export function BatchQueryButton({
  keys,
  draft,
  onChange,
}: {
  /** 当前页支持哪些字段（发票头页没有货物名称/税收分类编码） */
  keys: BatchKey[];
  draft: BatchDraft;
  onChange: (d: BatchDraft) => void;
}) {
  const [open, setOpen] = useState(false);
  const n = batchCount(draft);

  return (
    <>
      <AppButton type="button" variant="glass" onClick={() => setOpen(true)}>
        <ListBullets data-icon="inline-start" />
        批量查询{n > 0 && `（${n}）`}
      </AppButton>
      {/* 与综合查询的“选择列”同用 Drawer：Dialog 系遮罩带 backdrop-blur，关闭时整窗重绘会闪 */}
      <Drawer open={open} onOpenChange={setOpen} swipeDirection="right">
        <DrawerContent className="h-dvh w-[min(412px,100vw)]">
          <DrawerHeader className="border-b pb-3">
            <DrawerTitle>批量查询</DrawerTitle>
          </DrawerHeader>
          <div className="thin-scrollbar flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-4 py-3">
            {keys.map((k) => {
              const cur = draft[k] ?? { text: "", exclude: false };
              const set = (p: Partial<typeof cur>) => onChange({ ...draft, [k]: { ...cur, ...p } });
              const count = parseBatchText(cur.text, k).length;
              return (
                <div key={k} className="flex flex-col gap-1">
                  <div className="flex items-center justify-between">
                    <Label>
                      {BATCH_LABELS[k]}
                      {count > 0 && <span className="ml-2 text-muted-foreground">{count} 个</span>}
                    </Label>
                    <label className="flex items-center gap-2 text-muted-foreground">
                      {cur.exclude ? "不在清单中" : "在清单中"}
                      <Switch size="sm" checked={cur.exclude} onCheckedChange={(v) => set({ exclude: v })} />
                    </label>
                  </div>
                  <Textarea
                    rows={4}
                    value={cur.text}
                    onChange={(e) => set({ text: e.target.value })}
                    placeholder={
                      k === "goodsName"
                        ? "用逗号（中英文均可）或换行分隔；货物名称可含空格，空格不作分隔"
                        : "用空格、逗号（中英文均可）或换行分隔"
                    }
                    className="font-mono"
                  />
                </div>
              );
            })}
          </div>
          <DrawerFooter className="flex-row justify-end border-t pt-3">
            <AppButton type="button" variant="glass" onClick={() => onChange({})}>
              清空
            </AppButton>
            <AppButton type="button" variant="brand" onClick={() => setOpen(false)}>
              完成
            </AppButton>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>
    </>
  );
}

/** 查询表单里的「批量查询」按钮（页面里放在基础字段区末尾）。 */
export const BatchQueryField = BatchQueryButton;
