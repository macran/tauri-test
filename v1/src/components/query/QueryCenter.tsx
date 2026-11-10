// src/components/query/QueryCenter.tsx
//
// 所有查询页面的入口：左侧分类导航 + 右侧对应查询页面。
// 每个 Tab 内容只在被访问过之后才挂载（避免一次性发起 14 个查询页面的
// 初始渲染开销），访问过后保持挂载（切回去不丢失查询条件/结果）。

import { useState } from "react";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import AirDetailQuery from "./pages/AirDetailQuery";
import ApVoucherQuery from "./pages/ApVoucherQuery";
import BankVoucherQuery from "./pages/BankVoucherQuery";
import BuildingDetailQuery from "./pages/BuildingDetailQuery";
import CargoDetailQuery from "./pages/CargoDetailQuery";
import CertifiedListQuery from "./pages/CertifiedListQuery";
import IcVoucherQuery from "./pages/IcVoucherQuery";
import ItemOverviewQuery from "./pages/ItemOverviewQuery";
import InvoiceDetailQuery from "./pages/InvoiceDetailQuery";
import InvoiceHeaderQuery from "./pages/InvoiceHeaderQuery";
import PassengerDetailQuery from "./pages/PassengerDetailQuery";
import RailwayDetailQuery from "./pages/RailwayDetailQuery";
import RealEstateDetailQuery from "./pages/RealEstateDetailQuery";
import TollDetailQuery from "./pages/TollDetailQuery";
import TravelExpenseQuery from "./pages/TravelExpenseQuery";

const TABS = [
  { value: "overview", label: "综合查询", Component: ItemOverviewQuery },
  { value: "invoice-header", label: "发票头", Component: InvoiceHeaderQuery },
  { value: "invoice-detail", label: "发票明细", Component: InvoiceDetailQuery },
  { value: "building", label: "建筑服务", Component: BuildingDetailQuery },
  { value: "real-estate", label: "不动产租赁", Component: RealEstateDetailQuery },
  { value: "passenger", label: "旅客运输", Component: PassengerDetailQuery },
  { value: "cargo", label: "货物运输", Component: CargoDetailQuery },
  { value: "air", label: "航空运输", Component: AirDetailQuery },
  { value: "railway", label: "铁路客票", Component: RailwayDetailQuery },
  { value: "toll", label: "通行费", Component: TollDetailQuery },
  { value: "certified", label: "认证清单", Component: CertifiedListQuery },
  { value: "travel-expense", label: "差旅费用", Component: TravelExpenseQuery },
  { value: "bank", label: "银行回单凭证", Component: BankVoucherQuery },
  { value: "ap", label: "AP 应付凭证", Component: ApVoucherQuery },
  { value: "ic", label: "IC 凭证", Component: IcVoucherQuery },
] as const;

export default function QueryCenter() {
  const [visited, setVisited] = useState<Set<string>>(
    () => new Set([TABS[0].value]),
  );

  return (
    <Tabs
      defaultValue={TABS[0].value}
      onValueChange={(value) => {
        if (typeof value === "string") {
          setVisited((prev) => (prev.has(value) ? prev : new Set(prev).add(value)));
        }
      }}
      className="min-h-screen bg-muted/20"
    >
      <div className="sticky top-0 z-10 border-b bg-background px-6 py-3">
        <h1 className="mb-2 text-sm font-semibold">数据查询中心</h1>
        <TabsList variant="line" className="flex-wrap">
          {TABS.map((t) => (
            <TabsTrigger key={t.value} value={t.value}>
              {t.label}
            </TabsTrigger>
          ))}
        </TabsList>
      </div>

      {TABS.map(({ value, Component }) => (
        <TabsContent key={value} value={value}>
          {visited.has(value) && <Component />}
        </TabsContent>
      ))}
    </Tabs>
  );
}
