import { ChevronDown, Loader2, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { toast } from "@/components/ui/toast";

import { TaxRuleFormDialog } from "./components/TaxRuleFormDialog";
import { TaxRulesTable } from "./components/TaxRulesTable";
import { scopeLabel } from "./rules.types";

function getErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "string") {
    return error;
  }

  return "未知错误";
}

const scopeFilterOptions: Array<{ value: string; label: string }> = [
  { value: "input_tax_transfer", label: scopeLabel("input_tax_transfer") },
  { value: "stamp_tax", label: scopeLabel("stamp_tax") },
];

const reviewFilterOptions: Array<{ value: string; label: string }> = [
  { value: "true", label: "是" },
  { value: "false", label: "否" },
];

const statusFilterOptions: Array<{ value: string; label: string }> = [
  { value: "enabled", label: "已启用" },
  { value: "disabled", label: "已禁用" },
];

function FacetFilter({
  label,
  options,
  selected,
  onChange,
}: {
  label: string;
  options: Array<{ value: string; label: string }>;
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
}) {
  function toggle(value: string, checked: boolean) {
    const next = new Set(selected);

    if (checked) {
      next.add(value);
    } else {
      next.delete(value);
    }

    onChange(next);
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
        {selected.size === 0
          ? `请选择${label}`
          : `${label}（${selected.size}）`}
        <ChevronDown className="ml-1 size-3.5" />
      </DropdownMenuTrigger>

      <DropdownMenuContent>
        {options.map((option) => (
          <DropdownMenuCheckboxItem
            key={option.value}
            checked={selected.has(option.value)}
            onCheckedChange={(checked) => toggle(option.value, checked)}
          >
            {option.label}
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export default function TaxRulesPage() {
  const [rules, setRules] = useState<TaxMatchRuleRecord[]>([]);

  const [stampCategories, setStampCategories] = useState<
    StampTaxCategoryOption[]
  >([]);

  const [searchText, setSearchText] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const [scopeFilter, setScopeFilter] = useState<Set<string>>(new Set());
  const [reviewFilter, setReviewFilter] = useState<Set<string>>(new Set());
  const [statusFilter, setStatusFilter] = useState<Set<string>>(new Set());

  const [formOpen, setFormOpen] = useState(false);
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

  const filteredRules = useMemo(() => {
    const keyword = searchText.trim().toLowerCase();

    return rules.filter((rule) => {
      if (scopeFilter.size > 0 && !scopeFilter.has(rule.scope)) {
        return false;
      }

      if (
        reviewFilter.size > 0 &&
        !reviewFilter.has(String(rule.needsManualReview))
      ) {
        return false;
      }

      if (
        statusFilter.size > 0 &&
        !statusFilter.has(rule.enabled ? "enabled" : "disabled")
      ) {
        return false;
      }

      if (!keyword) {
        return true;
      }

      const text = [
        rule.scope,
        rule.ruleCategory,
        rule.sellerTaxIdValues ?? "",
        rule.taxCodeValues ?? "",
        rule.taxNameValues ?? "",
        rule.note,
      ]
        .join(" ")
        .toLowerCase();

      return text.includes(keyword);
    });
  }, [rules, searchText, scopeFilter, reviewFilter, statusFilter]);

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
      <Card className="h-full">
        <CardHeader className="flex flex-row items-center justify-between gap-4">
          <div>
            <CardTitle>规则管理</CardTitle>
            <p className="mt-1 text-sm text-muted-foreground">
              查询、添加和禁用进项税转出及印花税计算规则。
            </p>
          </div>

          <Button type="button" onClick={() => setFormOpen(true)}>
            <Plus className="mr-2 size-4" />
            添加规则
          </Button>
        </CardHeader>

        <CardContent className="space-y-4">
          <div className="flex flex-col gap-3 md:flex-row md:items-center">
            <Input
              value={searchText}
              onChange={(event) => setSearchText(event.target.value)}
              placeholder="搜索规则所属、规则类别、匹配条件或备注"
              className="md:max-w-md"
            />

            <FacetFilter
              label="规则所属"
              options={scopeFilterOptions}
              selected={scopeFilter}
              onChange={setScopeFilter}
            />

            <FacetFilter
              label="是否核对"
              options={reviewFilterOptions}
              selected={reviewFilter}
              onChange={setReviewFilter}
            />

            <FacetFilter
              label="状态"
              options={statusFilterOptions}
              selected={statusFilter}
              onChange={setStatusFilter}
            />

            <Badge variant="secondary" className="w-fit">
              共 {filteredRules.length} 条
            </Badge>
          </div>

          <Separator />

          <TaxRulesTable
            data={filteredRules}
            isLoading={isLoading}
            onDisable={setRuleToDisable}
          />
        </CardContent>
      </Card>

      <TaxRuleFormDialog
        open={formOpen}
        stampCategories={stampCategories}
        onOpenChange={setFormOpen}
        onSaved={async () => {
          setFormOpen(false);
          await loadRules();
        }}
      />

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
