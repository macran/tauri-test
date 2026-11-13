// src/components/query/DataTableMenus.tsx
//
// DataTable 工具栏上的布局弹层（保存/切换/删除）。列设置见 ColumnConfigurator。

import { FloppyDisk, Star, Trash } from "@phosphor-icons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import type { LayoutItem } from "./table-layout";

export function LayoutMenu({
  mine,
  selected,
  onSave,
  onSetDefault,
  onDelete,
}: {
  /** 当前用户自己的布局（他人的公开布局只能选用，不在这里管理） */
  mine: LayoutItem[];
  selected: LayoutItem | undefined;
  onSave: (name: string, isPrivate: boolean) => void;
  onSetDefault: (id: number | null) => void;
  onDelete: (id: number) => void;
}) {
  const [name, setName] = useState("");
  const [isPrivate, setIsPrivate] = useState(false);

  return (
    <Popover
      onOpenChange={(open) => {
        if (!open) return;
        const own = selected?.isMine ? selected : undefined;
        setName(own?.name ?? "");
        setIsPrivate(own?.isPrivate ?? false);
      }}
    >
      <PopoverTrigger render={<Button type="button" variant="outline" size="sm" />}>
        <FloppyDisk data-icon="inline-start" />
        保存布局
      </PopoverTrigger>
      <PopoverContent align="end" className="w-72">
        <div className="flex gap-1">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="布局名称（同名覆盖）"
          />
          <Button
            type="button"
            size="sm"
            disabled={!name.trim()}
            onClick={() => onSave(name.trim(), isPrivate)}
          >
            保存
          </Button>
        </div>
        <label className="flex items-center gap-2">
          <Checkbox checked={isPrivate} onCheckedChange={(c) => setIsPrivate(c === true)} />
          仅自己可见（否则所有人可用）
        </label>
        {mine.length > 0 && (
          <div className="border-t pt-2">
            {mine.map((l) => (
              <div
                key={l.id}
                className={cn("flex items-center gap-1 px-1", selected?.id === l.id && "bg-muted")}
              >
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate py-1 text-left"
                  onClick={() => {
                    setName(l.name);
                    setIsPrivate(l.isPrivate);
                  }}
                >
                  {l.name}
                  {l.isPrivate && <span className="text-muted-foreground">（私有）</span>}
                </button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={l.isDefault ? "取消默认" : "设为默认"}
                  title={l.isDefault ? "取消默认" : "设为我的默认布局"}
                  className={l.isDefault ? "text-primary" : "text-muted-foreground"}
                  onClick={() => onSetDefault(l.isDefault ? null : l.id)}
                >
                  <Star weight={l.isDefault ? "fill" : "regular"} />
                </Button>
                <Button type="button" variant="ghost" size="icon-xs" onClick={() => onDelete(l.id)}>
                  <Trash />
                </Button>
              </div>
            ))}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
