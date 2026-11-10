import TabbedPage from '@/components/TabbedPage'
import ApVoucherImportPage from '@/features/ap-voucher-import/ApVoucherImportPage'
import BankVoucherImportPage from '@/features/bank-voucher-import/BankVoucherImportPage'
import CertifiedImportPage from '@/features/certified-import/CertifiedImportPage'
import IcVoucherImportPage from '@/features/ic-voucher-import/IcVoucherImportPage'
import InvoiceImportPage from '@/features/invoice-import/InvoiceImportPage'
import TravelExpenseImportPage from '@/features/travel-expense-import/TravelExpenseImportPage'

const TAX = [
  { value: 'invoice', label: '全量发票', Component: InvoiceImportPage },
  { value: 'certified', label: '认证清单', Component: CertifiedImportPage },
]

const VOUCHER = [
  { value: 'ap', label: '应付凭证', Component: ApVoucherImportPage },
  { value: 'bank', label: '银行费用凭证', Component: BankVoucherImportPage },
  { value: 'ic', label: '内部关联方凭证', Component: IcVoucherImportPage },
  { value: 'travel', label: '差旅费用凭证', Component: TravelExpenseImportPage },
]

export const TaxImportHub = () => <TabbedPage title="税务数据导入" tabs={TAX} />
export const VoucherImportHub = () => <TabbedPage title="凭证数据导入" tabs={VOUCHER} />
