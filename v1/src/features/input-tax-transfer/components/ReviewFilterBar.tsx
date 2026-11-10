import type { ReviewRow } from "@/bindings";

import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";

export type ReviewFilter = {
  matchState: ReviewRow["matchState"] | "all";
  decisionSource: ReviewRow["decisionSource"] | "all";
  onlyNeedsReview: boolean;
  onlyIncluded: boolean;
};

export const DEFAULT_FILTER: ReviewFilter = {
  matchState: "all",
  decisionSource: "all",
  onlyNeedsReview: false,
  onlyIncluded: false,
};

export function applyFilter(rows: ReviewRow[], filter: ReviewFilter): ReviewRow[] {
  return rows.filter((row) => {
    if (filter.matchState !== "all" && row.matchState !== filter.matchState) {
      return false;
    }
    if (filter.decisionSource !== "all" && row.decisionSource !== filter.decisionSource) {
      return false;
    }
    if (filter.onlyNeedsReview && !row.needsManualReview) return false;
    if (filter.onlyIncluded && !row.finalIncluded) return false;
    return true;
  });
}

const MATCH_STATE_OPTIONS = [
  { value: "all", label: "全部" },
  { value: "matched", label: "已命中" },
  { value: "notMatched", label: "未命中" },
  { value: "missingVoucher", label: "缺凭证号" },
] as const;

const SOURCE_OPTIONS = [
  { value: "all", label: "全部" },
  { value: "system", label: "机器匹配" },
  { value: "manual", label: "人工修改" },
] as const;

type ReviewFilterBarProps = {
  filter: ReviewFilter;
  onChange: (filter: ReviewFilter) => void;
  visibleCount: number;
  totalCount: number;
};

export function ReviewFilterBar({
  filter,
  onChange,
  visibleCount,
  totalCount,
}: ReviewFilterBarProps) {
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-sm">
      <div className="flex items-center gap-2">
        <span className="text-muted-foreground">匹配状态</span>
        <ToggleGroup
          type="single"
          value={filter.matchState}
          onValueChange={(value) =>
            value &&
            onChange({ ...filter, matchState: value as ReviewFilter["matchState"] })
          }
        >
          {MATCH_STATE_OPTIONS.map((option) => (
            <ToggleGroupItem key={option.value} value={option.value} className="h-8 px-3">
              {option.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <div className="flex items-center gap-2">
        <span className="text-muted-foreground">结论来源</span>
        <ToggleGroup
          type="single"
          value={filter.decisionSource}
          onValueChange={(value) =>
            value &&
            onChange({
              ...filter,
              decisionSource: value as ReviewFilter["decisionSource"],
            })
          }
        >
          {SOURCE_OPTIONS.map((option) => (
            <ToggleGroupItem key={option.value} value={option.value} className="h-8 px-3">
              {option.label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>

      <Button
        type="button"
        size="sm"
        variant={filter.onlyNeedsReview ? "default" : "outline"}
        onClick={() => onChange({ ...filter, onlyNeedsReview: !filter.onlyNeedsReview })}
      >
        仅需人工核对
      </Button>

      <Button
        type="button"
        size="sm"
        variant={filter.onlyIncluded ? "default" : "outline"}
        onClick={() => onChange({ ...filter, onlyIncluded: !filter.onlyIncluded })}
      >
        仅已转出
      </Button>

      <span className="ml-auto text-muted-foreground">
        显示 {visibleCount} / {totalCount} 条
      </span>

      <Button
        type="button"
        size="sm"
        variant="ghost"
        onClick={() => onChange(DEFAULT_FILTER)}
      >
        重置筛选
      </Button>
    </div>
  );
}