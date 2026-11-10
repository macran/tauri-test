// src/lib/enum-options.ts
//
// 查询页面下拉选项。数值与 src-tauri/src/models/enums.rs 中各枚举的
// #[repr(u8)] 判别值一一对应——后端查询 DTO 里这些字段都是裸 u8，
// 不是枚举类型本身，所以这里手工维护中文标签。

export interface EnumOption {
  value: number;
  label: string;
}

export const INV_TYPE_OPTIONS: EnumOption[] = [
  { value: 1, label: "数电发票（普通发票）" },
  { value: 2, label: "数电发票（增值税专用发票）" },
  { value: 3, label: "增值税专用发票" },
  { value: 4, label: "增值税电子普通发票" },
  { value: 5, label: "数电纸质发票（普通发票）" },
  { value: 6, label: "增值税普通发票" },
  { value: 7, label: "数电纸质发票（增值税专用发票）" },
  { value: 8, label: "增值税普通发票（卷式）" },
  { value: 9, label: "道路通行费电子普通发票" },
  { value: 10, label: "数电发票（铁路电子客票）" },
  { value: 11, label: "数电发票（航空运输电子客票行程单）" },
  { value: 12, label: "数电发票（通行费发票）" },
  { value: 13, label: "增值税电子专用发票" },
];

export const INV_STATE_OPTIONS: EnumOption[] = [
  { value: 1, label: "正常" },
  { value: 2, label: "已作废" },
  { value: 3, label: "已红冲-全额" },
  { value: 4, label: "已红冲-部分" },
  { value: 5, label: "异常" },
];

export const IS_POSITIVE_OPTIONS: EnumOption[] = [
  { value: 1, label: "是（蓝字）" },
  { value: 0, label: "否（红字）" },
];

export const RISK_LEVEL_OPTIONS: EnumOption[] = [
  { value: 1, label: "正常" },
  { value: 2, label: "疑点" },
  { value: 3, label: "风险" },
  { value: 4, label: "高危" },
];

export const INV_SOURCE_OPTIONS: EnumOption[] = [
  { value: 1, label: "电子发票服务平台" },
  { value: 2, label: "增值税发票管理系统" },
];

export const SPECIAL_BIZ_TYPE_OPTIONS: EnumOption[] = [
  { value: 1, label: "铁路电子客票" },
  { value: 2, label: "旅客运输服务发票" },
  { value: 3, label: "通行费" },
  { value: 4, label: "成品油发票" },
  { value: 5, label: "农产品" },
  { value: 6, label: "货物运输服务发票" },
  { value: 7, label: "不动产经营租赁服务" },
  { value: 8, label: "航空运输电子客票行程单" },
  { value: 9, label: "建筑服务发票" },
];

export const CERT_RISK_STATUS_OPTIONS: EnumOption[] = [
  { value: 0, label: "无风险" },
  { value: 1, label: "蓝色预警" },
  { value: 2, label: "黄色预警" },
  { value: 3, label: "红色预警" },
];

export const AP_INV_TYPE_OPTIONS: EnumOption[] = [
  { value: 0, label: "增票" },
  { value: 1, label: "普票" },
  { value: 2, label: "红票" },
  { value: 3, label: "收据" },
  { value: 4, label: "票据" },
  { value: 5, label: "通知书" },
];

export const TAX_CERTIFIED_OPTIONS: EnumOption[] = [
  { value: 1, label: "是" },
  { value: 0, label: "否" },
];

export function enumLabel(options: EnumOption[], value: number | null | undefined): string {
  if (value === null || value === undefined) return "—";
  return options.find((o) => o.value === value)?.label ?? String(value);
}
