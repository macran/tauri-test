import { useForm, useSelector } from "@tanstack/react-form";
import { useEffect, useState } from "react";
import {
  commands,
  type StampTaxCategoryOption,
  type TaxMatchRuleRecord,
} from "@/bindings";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Textarea } from "@/components/ui/textarea";
import { toast } from "@/components/ui/toast";

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
};

function focusField(key: RuleFieldKey) {
  const element = document.getElementById(FIELD_ELEMENT_IDS[key]);

  if (!element) {
    return;
  }

  element.scrollIntoView({ block: "center", behavior: "smooth" });
  element.focus();
}

function FieldError({ message }: { message?: string }) {
  if (!message) {
    return null;
  }

  return <p className="text-xs text-destructive">{message}</p>;
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
  const stampCategoryItems = stampCategories.map((category) => ({
    value: category.categoryName,
    label: category.categoryName,
  }));
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
      <div className="grid gap-1 md:grid-cols-[180px_140px_1fr] md:items-start md:gap-3">
        <Label className="pt-2">{label}</Label>

        <div className="space-y-1">
          <form.Field name={typeName}>
            {(field) => (
              <Select
                items={matchTypeOptions}
                value={field.state.value || null}
                onValueChange={(value) => {
                  const nextValue = (value ?? "") as MatchType;

                  field.handleChange(nextValue);
                  clearFieldError(typeName);

                  if (nextValue === "") {
                    form.setFieldValue(valueName, "");
                  }
                }}
              >
                <SelectTrigger
                  id={FIELD_ELEMENT_IDS[typeName]}
                  aria-invalid={fieldErrors[typeName] !== undefined}
                >
                  <SelectValue placeholder="不参与匹配" />
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

        <div className="space-y-1">
          <form.Field name={valueName}>
            {(field) => (
              <Input
                id={FIELD_ELEMENT_IDS[valueName]}
                value={field.state.value}
                aria-invalid={fieldErrors[valueName] !== undefined}
                onBlur={field.handleBlur}
                onChange={(event) => {
                  field.handleChange(event.target.value);
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
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>添加规则</DialogTitle>
          <DialogDescription>
            未选择的匹配维度将保存为空值。多个匹配维度之间是 AND 关系。
          </DialogDescription>
        </DialogHeader>

        <form
          className="space-y-6"
          onSubmit={(event) => {
            event.preventDefault();
            event.stopPropagation();
            handleSubmit();
          }}
        >
          <div className="grid gap-4 md:grid-cols-2">
            <form.Field name="scope">
              {(field) => (
                <div className="space-y-2">
                  <Label htmlFor="rule-scope">
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
                    <SelectTrigger id="rule-scope">
                      <SelectValue placeholder="选择规则所属" />
                    </SelectTrigger>

                    <SelectContent>
                      {scopeOptions.map((option) => (
                        <SelectItem key={option.value} value={option.value}>
                          {option.label}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              )}
            </form.Field>

            {scope === "input_tax_transfer" ? (
              <form.Field name="ruleCategory">
                {(field) => (
                  <div className="space-y-2">
                    <Label htmlFor={FIELD_ELEMENT_IDS.ruleCategory}>
                      规则类别
                      <span className="ml-1 text-destructive">*</span>
                    </Label>

                    <Input
                      id={FIELD_ELEMENT_IDS.ruleCategory}
                      value={field.state.value}
                      aria-invalid={fieldErrors.ruleCategory !== undefined}
                      onBlur={field.handleBlur}
                      onChange={(event) => {
                        field.handleChange(event.target.value);
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
                  <div className="space-y-2">
                    <Label htmlFor={FIELD_ELEMENT_IDS.ruleCategory}>
                      印花税税目
                      <span className="ml-1 text-destructive">*</span>
                    </Label>

                    <Select
                      items={stampCategoryItems}
                      value={field.state.value || null}
                      onValueChange={(value) => {
                        field.handleChange(value ?? "");
                        clearFieldError("ruleCategory");
                      }}
                    >
                      <SelectTrigger
                        id={FIELD_ELEMENT_IDS.ruleCategory}
                        aria-invalid={fieldErrors.ruleCategory !== undefined}
                      >
                        <SelectValue placeholder="请选择印花税税目" />
                      </SelectTrigger>

                      <SelectContent>
                        {stampCategoryItems.map((item) => (
                          <SelectItem key={item.value} value={item.value}>
                            {item.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>

                    <FieldError message={fieldErrors.ruleCategory} />
                  </div>
                )}
              </form.Field>
            )}
          </div>

          <Separator />

          <section className="space-y-3">
            <div>
              <h3 className="text-sm font-medium">匹配条件</h3>
              <p className="text-sm text-muted-foreground">
                可以同时填写多个维度。未选择的维度将保存为 NULL。
              </p>
            </div>

            {renderMatchField(
              "sellerTaxIdMatchType",
              "sellerTaxIdValues",
              "销售方税号",
              "请输入销售方税号"
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
          </section>

          <Separator />

          <form.Field name="needsManualReview">
            {(field) => (
              <label
                htmlFor="check"
                className="flex items-center gap-2 text-sm"
              >
                <Checkbox
                  id="is-check"
                  checked={field.state.value}
                  onCheckedChange={(checked) =>
                    field.handleChange(checked === true)
                  }
                />
                命中后需要人工核对
              </label>
            )}
          </form.Field>

          <form.Field name="note">
            {(field) => (
              <div className="space-y-2">
                <Label htmlFor="rule-note">备注</Label>

                <Textarea
                  id="rule-note"
                  value={field.state.value}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  placeholder="请输入备注，可为空"
                  rows={3}
                />
              </div>
            )}
          </form.Field>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
            </Button>

            <Button type="submit">保存规则</Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
