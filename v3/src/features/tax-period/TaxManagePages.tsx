// 进项转出管理 / 印花税管理 / 红冲管理：页标题 + 标签组，每个标签一个独立子页。
import TabbedPage from '@/components/TabbedPage'
import ReversalMatch from '@/components/ReversalMatch'
import ReversalQueryPage from '@/features/reversal-query/ReversalQueryPage'
import InputTaxTransferPage from '@/features/input-tax-transfer/InputTaxTransferPage'
import TransferQueryPage from '@/features/input-tax-transfer/TransferQueryPage'
import StampTaxPage from '@/features/stamp-tax/StampTaxPage'
import StampTaxQueryPage from '@/features/stamp-tax/StampTaxQueryPage'

const TRANSFER = [
  { value: 'match', label: '转出匹配', Component: InputTaxTransferPage },
  { value: 'query', label: '转出查询', Component: TransferQueryPage },
]
const STAMP = [
  { value: 'match', label: '印花税匹配', Component: StampTaxPage },
  { value: 'query', label: '印花税查询', Component: StampTaxQueryPage },
]

const REVERSAL = [
  { value: 'match', label: '红冲核对', Component: ReversalMatch },
  { value: 'query', label: '红冲查询', Component: ReversalQueryPage },
]

export const InputTaxTransferManage = () => <TabbedPage title="进项转出管理" tabs={TRANSFER} />
export const StampTaxManage = () => <TabbedPage title="印花税管理" tabs={STAMP} />
export const ReversalManage = () => <TabbedPage title="红冲管理" tabs={REVERSAL} />
