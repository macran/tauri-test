import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";

interface Props {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  title: string;
  description: string;
  confirmText: string;
  requireReason?: boolean;
  onSubmit: (reason: string) => Promise<void>;
}

export function AdminReasonDialog({ open, onOpenChange, title, description, confirmText, requireReason = true, onSubmit }: Props) {
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const invalid = requireReason && !reason.trim();
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        if (!o) setReason("");
        onOpenChange(o);
      }}
    >
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <div className="grid gap-1.5">
          <Label htmlFor="admin-reason">原因{requireReason && <span className="ml-0.5 text-destructive">*</span>}</Label>
          <textarea
            id="admin-reason"
            rows={3}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            placeholder="例如：2026-11 税务局核查，202608 期需补转出"
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            disabled={invalid || busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onSubmit(reason.trim());
                setReason("");
              } finally {
                setBusy(false);
              }
            }}
          >
            {busy ? "处理中…" : confirmText}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
