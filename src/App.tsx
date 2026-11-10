import ReversalMatch from "./components/ReversalMatch";
import RuleExcelImport from "./components/RuleExcelImport";
import { Toaster } from "./components/ui/toast";
import ApVoucherImportPage from "./features/ap-voucher-import/ApVoucherImportPage";
import BankVoucherImportPage from "./features/bank-voucher-import/BankVoucherImportPage";
import CertifiedImportPage from "./features/certified-import/CertifiedImportPage";
import IcVoucherImportPage from "./features/ic-voucher-import/IcVoucherImportPage";
import InputTaxTransferPage from "./features/input-tax-transfer/InputTaxTransferPage";
import InvoiceImportPage from "./features/invoice-import/InvoiceImportPage";
import TaxRulesPage from "./features/rules/TaxRulesPage";
import TravelExpenseImportPage from "./features/travel-expense-import/TravelExpenseImportPage";

const App = () => {
  return (
    <>
      <InvoiceImportPage />
      <CertifiedImportPage />
      <ReversalMatch />
      <TravelExpenseImportPage />
      <BankVoucherImportPage />
      <ApVoucherImportPage />
      <IcVoucherImportPage />
      <RuleExcelImport />
      <TaxRulesPage />
      <InputTaxTransferPage />
      <Toaster />
    </>
  );
};
export default App;
