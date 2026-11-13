import { createFileRoute } from '@tanstack/react-router'
import { VoucherImportHub } from '../features/import-hub/ImportHubs'

export const Route = createFileRoute('/upload_business')({
  component: VoucherImportHub,
})
