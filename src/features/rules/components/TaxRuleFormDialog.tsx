import { FloppyDisk, X } from "@phosphor-icons/react";
import { useForm, useSelector } from "@tanstack/react-form";
import { useEffect, useState } from "react";
import {
  commands,
  type StampTaxCategoryOption,
  type TaxMatchRuleRecord,
} from "@/bindings";

import { AppButton } from "@/components/app-button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";
import { ClearableInput } from "@/features/query/fields";
import { StampCategoryCombobox } from "./StampCategoryCombobox";
import { cn } from "@/lib/utils";

import {
  defaultRuleFormValues,
  type MatchType,
  type RuleFieldKey,
  type RuleFormValues,
  validateRuleFields,
} from "../rules.types";

type TaxRuleFormDialogProps = {
  open: boolean;
  stampCategories: StampTaxCategoryOption[];
  onOpenChange: (open: boolean) => void;
  onSaved: (rule: TaxMatchRuleRecord) => Promise<void>;
};

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "未知错误";
}

function toOptional(value: string): string | null {
  const trimmed = value.trim();

  return trimmed === "" ? null : trimmed;
}

const FIELD_ELEMENT_IDS: Record<RuleFieldKey, string> = {
  ruleCategory: "rule-category-field",
  sellerTaxIdMatchType: "seller-tax-id-match-type",
  sellerTaxIdValues: "seller-tax-id-values",
  taxCodeMatchType: "tax-code-match-type",
  taxCodeValues: "tax-code-values",
  taxNameMatchType: "tax-name-match-type",
  taxNameValues: "tax-name-values",
  // 没有任何匹配条件时，定位到第一个匹配方式下拉
  matchConditions: "seller-tax-id-match-type",
};

function focusField(key: RuleFieldKey) {
  const element = document.getElementById(FIELD_ELEMENT_IDS[key]);

  if (!element) {
    return;
  }

  element.scrollIntoView({ block: "center", behavior: "smooth" });
  element.focus();
}

// 固定高度的错误行：错误出现/消失时弹窗不会被撑高
function FieldError({ message }: { message?: string }) {
  return (
    <p className={cn("h-4 truncate text-xs text-destructive", !message && "invisible")}>
      {message ?? "-"}
    </p>
  );
}

const scopeOptions = [
  {
    value: "input_tax_transfer",
    label: "进项税转出",
  },
  {
    value: "stamp_tax",
    label: "印花税计算",
  },
];

const matchTypeOptions = [
  {
    value: null,
    label: "不参与匹配",
  },
  {
    value: "eq",
    label: "等于",
  },
  {
    value: "neq",
    label: "不等于",
  },
];

export function TaxRuleFormDialog({
  open,
  stampCategories,
  onOpenChange,
  onSaved,
}: TaxRuleFormDialogProps) {
  const [fieldErrors, setFieldErrors] = useState<
    Partial<Record<RuleFieldKey, string>>
  >({});

  const form = useForm({
    defaultValues: defaultRuleFormValues,
  });
  const scope = useSelector(form.store, (state) => state.values.scope);

  useEffect(() => {
    if (!open) {
      form.reset();
      setFieldErrors({});
    }
  }, [open, form]);

  function clearFieldError(key: RuleFieldKey) {
    setFieldErrors((previous) => {
      if (!previous[key]) {
        return previous;
      }

      const next = { ...previous };
      delete next[key];
      return next;
    });
  }

  // Fires the backend call after the dialog has already closed, so the
  // outcome (success/duplicate/failure) is only ever reported via toast.
  //
  // Driven manually (add + update) rather than via toast.promise(), since
  // that helper always forces type:"success" on a resolved promise and
  // type:"error" on a rejected one — it can't render our "duplicate" outcome
  // (a resolved value, not a rejection) with distinct warning styling.
  async function submitRule(value: RuleFormValues) {
    const toastId = toast.add({ type: "loading", title: "正在保存规则…" });

    try {
      const result = await commands.addTaxMatchRule({
        scope: value.scope,
        ruleCategory: value.ruleCategory.trim(),

        sellerTaxIdMatchType: value.sellerTaxIdMatchType || null,
        sellerTaxIdValues: toOptional(value.sellerTaxIdValues),

        taxCodeMatchType: value.taxCodeMatchType || null,
        taxCodeValues: toOptional(value.taxCodeValues),

        taxNameMatchType: value.taxNameMatchType || null,
        taxNameValues: toOptional(value.taxNameValues),

        needsManualReview: value.needsManualReview,
        note: value.note.trim(),
      });

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      const outcome = result.data;

      if (outcome.status === "duplicate") {
        toast.update(toastId, {
          type: "warning",
          title: "规则未保存",
          description: outcome.message,
        });
        return;
      }

      toast.update(toastId, { type: "success", title: "规则添加成功" });
      await onSaved(outcome.rule);
    } catch (error) {
      toast.update(toastId, {
        type: "error",
        title: "规则添加失败",
        description: getErrorMessage(error),
      });
    }
  }

  function handleSubmit() {
    const errors = validateRuleFields(form.state.values);
    const firstInvalidField = Object.keys(errors)[0] as
      | RuleFieldKey
      | undefined;

    setFieldErrors(errors);

    if (firstInvalidField) {
      focusField(firstInvalidField);
      return;
    }

    const value = form.state.values;

    onOpenChange(false);
    void submitRule(value);
  }

  function renderMatchField(
    typeName: "sellerTaxIdMatchType" | "taxCodeMatchType" | "taxNameMatchType",
    valueName: "sellerTaxIdValues" | "taxCodeValues" | "taxNameValues",
    label: string,
    placeholder: string
  ) {
    return (
      <div className="grid gap-x-3 md:grid-cols-[150px_140px_1fr] md:items-start">
        <Label className="h-(--control-h) text-xs text-muted-foreground">{label}</Label>

        <div className="flex flex-col gap-1">
          <form.Field name={typeName}>
            {(field) => (
              <Select
                items={matchTypeOptions}
                value={field.state.value || null}
                onValueChange={(value) => {
                  const nextValue = (value ?? "") as MatchType;

                  field.handleChange(nextValue);
                  clearFieldError(typeName);
                  clearFieldError(valueName);
                  clearFieldError("matchConditions");

                  if (nextValue === "") {
                    form.setFieldValue(valueName, "");
                  }
                }}
              >
                <SelectTrigger
                  className="w-full min-w-0"
                  id={FIELD_ELEMENT_IDS[typeName]}
                  aria-invalid={fieldErrors[typeName] !== undefined}
                >
                  <SelectValue className="min-w-0 truncate" placeholder="不参与匹配" />
                </SelectTrigger>

                <SelectContent>
                  {matchTypeOptions.map((option) => (
                    <SelectItem
                      key={option.value ?? "none"}
                      value={option.value}
                    >
                      {option.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            )}
          </form.Field>

          <FieldError message={fieldErrors[typeName]} />
        </div>

        <div className="flex flex-col gap-1">
          <form.Field name={valueName}>
            {(field) => (
              <ClearableInput
                id={FIELD_ELEMENT_IDS[valueName]}
                value={field.state.value}
                aria-invalid={fieldErrors[valueName] !== undefined}
                onBlur={field.handleBlur}
                onChange={(event) => {
                  field.handleChange(event.target.value);
                  clearFieldError(valueName);
                  clearFieldError("matchConditions");
                }}
                onClear={() => {
                  field.handleChange("");
                  clearFieldError(valueName);
                }}
                placeholder={placeholder}
              />
            )}
          </form.Field>

          <FieldError message={fieldErrors[valueName]} />
        </div>
      </div>
    );
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] gap-3 overflow-y-auto sm:max-w-3xl">
        <DialogHeader className="border-b pb-3">
          <DialogTitle>添加规则</DialogTitle>
        </DialogHeader>

        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            event.stopPropagation();
            handleSubmit();
          }}
        >
          {/* 前两列宽 = 匹配行的「标签 + 匹配方式」列，规则类别正好与下方匹配内容输入框左对齐 */}
          <div className="grid gap-x-3 md:grid-cols-[calc(150px+140px+0.75rem)_1fr]">
            <form.Field name="scope">
              {(field) => (
                <div className="flex flex-col gap-1">
                  <Label htmlFor="rule-scope" className="text-xs text-muted-foreground">
                    规则所属
                    <span className="ml-1 text-destructive">*</span>
                  </Label>

                  <Select
                    items={scopeOptions}
                    value={field.state.value || null}
                    onValueChange={(value) => {
                      if (value === null) {
                        return;
                      }

                      field.handleChange(value as RuleFormValues["scope"]);

                      form.setFieldValue("ruleCategory", "");
                      clearFieldError("ruleCategory");
                    }}
                  >
                    <SelectTrigger id="rule-scope" className="w-full min-w-0">
                      <SelectValue className="min-w-0 truncate" placeholder="选择规则所属" />
                    </SelectTrigger>

                    <SelectContent>
                      {scopeOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>

                  <FieldError />
                </div>
              )}
            </form.Field>

            {scope === "input_tax_transfer" ? (
              <form.Field name="ruleCategory">
                {(field) => (
                  <div className="flex flex-col gap-1">
                    <Label htmlFor={FIELD_ELEMENT_IDS.ruleCategory} className="text-xs text-muted-foreground">
                      规则类别
                      <span className="ml-1 text-destructive">*</span>
                    </Label>

                    <ClearableInput
                      id={FIELD_ELEMENT_IDS.ruleCategory}
                      value={field.state.value}
                      aria-invalid={fieldErrors.ruleCategory !== undefined}
                      onBlur={field.handleBlur}
                      onChange={(event) => {
                        field.handleChange(event.target.value);
                        clearFieldError("ruleCategory");
                      }}
                      onClear={() => {
                        field.handleChange("");
                        clearFieldError("ruleCategory");
                      }}
                      placeholder="例如：员工福利/个人消费"
                    />

                    <FieldError message={fieldErrors.ruleCategory} />
                  </div>
                )}
              </form.Field>
            ) : (
              <form.Field name="ruleCategory">
                {(field) => (
                  <div className="flex flex-col gap-1">
                    <Label htmlFor={FIELD_ELEMENT_IDS.ruleCategory} className="text-xs text-muted-foreground">
                      印花税税目
                      <span className="ml-1 text-destructive">*</span>
                    </Label>

                    <StampCategoryCombobox
                      id={FIELD_ELEMENT_IDS.ruleCategory}
                      options={stampCategories}
                      value={field.state.value}
                      invalid={fieldErrors.ruleCategory !== undefined}
                      onChange={(name) => {
                        field.handleChange(name);
                        clearFieldError("ruleCategory");
                      }}
                    />

                    <FieldError message={fieldErrors.ruleCategory} />
                  </div>
                )}
              </form.Field>
            )}
          </div>

          <section className="flex flex-col border-t pt-3">
            <div className="mb-3">
              <h3 className="text-xs font-semibold">匹配条件</h3>
              
            </div>

            {renderMatchField(
              "sellerTaxIdMatchType",
              "sellerTaxIdValues",
              "销售方税号",
              "请输入销售方税号,多个用逗号隔开"
            )}

            {renderMatchField(
              "taxCodeMatchType",
              "taxCodeValues",
              "税收分类编码",
              "请输入税收分类编码"
            )}

            {renderMatchField(
              "taxNameMatchType",
              "taxNameValues",
              "税收分类编码简称",
              "请输入税收分类编码简称"
            )}

            <FieldError message={fieldErrors.matchConditions} />
          </section>

          <form.Field name="needsManualReview">
            {(field) => (
              <Label className="flex w-full items-center gap-2 border-t pt-3 text-xs">
                <Checkbox
                  checked={field.state.value}
                  onCheckedChange={(checked) =>
                    field.handleChange(checked === true)
                  }
                />
                需要人工核对
              </Label>
            )}
          </form.Field>

          <form.Field name="note">
            {(field) => (
              <div className="flex flex-col gap-1">
                <Label htmlFor="rule-note" className="text-xs text-muted-foreground">备注</Label>

                <Textarea
                  id="rule-note"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="请输入备注，可为空"
                  rows={2}
                />
              </div>
            )}
          </form.Field>

          <DialogFooter>
            <AppButton
              type="button"
              variant="glass"
              onClick={() => onOpenChange(false)}
            >
              <X data-icon="inline-start" />
              取消
            </AppButton>

            <AppButton type="submit" variant="brand">
              <FloppyDisk data-icon="inline-start" weight="bold" />
              保存规则
            </AppButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
