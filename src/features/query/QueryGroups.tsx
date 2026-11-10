// 数据查询的各大类：每类对应侧边栏一项，类内用下拉选择具体查询页（相当于 v1 QueryCenter 的 tab）。
import SubTabs from '@/components/SubTabs'
import AirDetailQuery from './pages/AirDetailQuery'
import ApVoucherQuery from './pages/ApVoucherQuery'
import BankVoucherQuery from './pages/BankVoucherQuery'
import BuildingDetailQuery from './pages/BuildingDetailQuery'
import CargoDetailQuery from './pages/CargoDetailQuery'
import CertifiedListQuery from './pages/CertifiedListQuery'
import IcVoucherQuery from './pages/IcVoucherQuery'
import InvoiceDetailQuery from './pages/InvoiceDetailQuery'
import InvoiceHeaderQuery from './pages/InvoiceHeaderQuery'
import ItemOverviewQuery from './pages/ItemOverviewQuery'
import PassengerDetailQuery from './pages/PassengerDetailQuery'
import RailwayDetailQuery from './pages/RailwayDetailQuery'
import RealEstateDetailQuery from './pages/RealEstateDetailQuery'
import TollDetailQuery from './pages/TollDetailQuery'
import TravelExpenseQuery from './pages/TravelExpenseQuery'

const INVOICE = [
  { value: 'header', label: '发票头', Component: InvoiceHeaderQuery },
  { value: 'detail', label: '发票明细', Component: InvoiceDetailQuery },
]
const INDUSTRY = [
  { value: 'building', label: '建筑服务', Component: BuildingDetailQuery },
  { value: 'real-estate', label: '不动产租赁', Component: RealEstateDetailQuery },
  { value: 'passenger', label: '旅客运输', Component: PassengerDetailQuery },
  { value: 'cargo', label: '货物运输', Component: CargoDetailQuery },
  { value: 'air', label: '航空运输', Component: AirDetailQuery },
  { value: 'railway', label: '铁路客票', Component: RailwayDetailQuery },
  { value: 'toll', label: '通行费', Component: TollDetailQuery },
]
const CERTIFIED = [{ value: 'certified', label: '认证清单', Component: CertifiedListQuery }]
const VOUCHER = [
  { value: 'ap', label: '应付凭证', Component: ApVoucherQuery },
  { value: 'ic', label: '关联方凭证', Component: IcVoucherQuery },
  { value: 'bank', label: '银行费用凭证', Component: BankVoucherQuery },
  { value: 'travel', label: '差旅费用凭证', Component: TravelExpenseQuery },
]
const OVERVIEW = [{ value: 'overview', label: '综合查询', Component: ItemOverviewQuery }]

export const InvoiceQuery = () => <SubTabs title="发票查询" entries={INVOICE} />
export const IndustryQuery = () => <SubTabs title="特定行业查询" entries={INDUSTRY} />
export const CertifiedQuery = () => <SubTabs title="认证清单查询" entries={CERTIFIED} />
export const VoucherQuery = () => <SubTabs title="凭证查询" entries={VOUCHER} />
export const OverviewQuery = () => <SubTabs title="综合查询" entries={OVERVIEW} />
