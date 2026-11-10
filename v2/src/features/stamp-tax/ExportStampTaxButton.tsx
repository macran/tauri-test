import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { fenToYuan, type StampTaxReport, useExportStampTaxReport } from "./api";

interface Props {
  /** 由上层根据用户选择的申报期间汇总得到 */
  report: StampTaxReport | undefined;
}

export function ExportStampTaxButton({ report }: Props) {
  const exportReport = useExportStampTaxReport();
  const disabled = !report || report.lines.length === 0 || exportReport.isPending;

  const handleClick = () => {
    if (!report) return;
    exportReport.mutate(report, {
      onSuccess: (result) => {
        if (!result) return; // 用户取消了保存对话框
        const { rowCount, totalTaxableFen, totalTaxFen } = result.summary;
        toast.success("采集表已生成", {
          description: `${rowCount} 行，计税金额 ${fenToYuan(totalTaxableFen)} 元，应纳税额 ${fenToYuan(totalTaxFen)} 元`,
        });
      },
      // Rust 端错误已序列化为中文提示字符串
      onError: (err) => toast.error("生成失败", { description: String(err) }),
    });
  };

  return (
    <Button onClick={handleClick} disabled={disabled}>
      {exportReport.isPending ? "正在生成…" : "生成印花税采集表"}
    </Button>
  );
}
