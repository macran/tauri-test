import { Check, RotateCcw, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";

export type BulkAction = "include" | "exclude" | "reset";

type ReviewBulkActionBarProps = {
  selectedCount: number;
  isSaving: boolean;
  onApply: (action: BulkAction) => void;
  onClearSelection: () => void;
};

/** 仅在有选中行时出现，避免常驻占位。 */
export function ReviewBulkActionBar({
  selectedCount,
  isSaving,
  onApply,
  onClearSelection,
}: ReviewBulkActionBarProps) {
  if (selectedCount === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm">
      <span className="font-medium">已选 {selectedCount} 条</span>

      <Separator orientation="vertical" className="h-5" />

      <Button
        type="button"
        size="sm"
        disabled={isSaving}
        onClick={() => onApply("include")}
      >
        <Check className="mr-1.5 size-4" />
        标记为计入印花税
      </Button>

      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={isSaving}
        onClick={() => onApply("exclude")}
      >
        <X className="mr-1.5 size-4" />
        标记为不计入印花税
      </Button>

      <Button
        type="button"
        size="sm"
        variant="outline"
        disabled={isSaving}
        onClick={() => onApply("reset")}
      >
        <RotateCcw className="mr-1.5 size-4" />
        恢复系统结论
      </Button>

      <Button
        type="button"
        size="sm"
        variant="ghost"
        className="ml-auto"
        onClick={onClearSelection}
      >
        取消选择
      </Button>
    </div>
  );
}
