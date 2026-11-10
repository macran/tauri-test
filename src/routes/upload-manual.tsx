import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/upload-manual')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/upload-manual"!</div>
}
