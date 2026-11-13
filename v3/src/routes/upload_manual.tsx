import { createFileRoute } from '@tanstack/react-router'
import InputTaxTransferPage from '../features/input-tax-transfer/InputTaxTransferPage'

export const Route = createFileRoute('/upload_manual')({
  component: InputTaxTransferPage,
})
