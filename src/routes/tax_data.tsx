import { createFileRoute } from '@tanstack/react-router'
import { TaxImportHub } from '../features/import-hub/ImportHubs'

export const Route = createFileRoute('/tax_data')({
  component: TaxImportHub,
})
