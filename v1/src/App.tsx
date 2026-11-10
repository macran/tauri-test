import UserManagement from "./components/admin/UserManagement";
import ApVoucherImport from "./components/ApVoucherImport";
import BankVoucherImport from "./components/BankVoucherImport";
import IcVoucherImport from "./components/IcVoucherImport";
import ImportCertified from "./components/ImportCertified";
import InvoiceImport from "./components/ImportExcel";
import QueryCenter from "./components/query/QueryCenter";
import ReversalMatch from "./components/ReversalMatch";
import RuleExcelImport from "./components/RuleExcelImport";
import TravelExpenseImport from "./components/TravelExpenseImport";
import { Toaster } from "./components/ui/toast";
import InputTaxTransferPage from "./features/input-tax-transfer/InputTaxTransferPage";
import TaxRulesPage from "./features/rules/TaxRulesPage";
import StampTaxPage from "./features/stamp-tax/StampTaxPage";

const App = () => {
  return (
    <>
      <InvoiceImport />
      <ImportCertified />
      <ReversalMatch />
      <TravelExpenseImport />
      <BankVoucherImport />
      <ApVoucherImport />
      <IcVoucherImport />
      <RuleExcelImport />
      <TaxRulesPage />
      <InputTaxTransferPage />
      <StampTaxPage />
      <QueryCenter />
      <UserManagement />
      <Toaster />
    </>
  );
};
export default App;
