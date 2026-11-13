// 主窗口入口：router、QueryClient 和所有业务页面都在这里，由 main.tsx 懒加载。
import { createRouter } from "@tanstack/react-router"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"

import { Toaster } from "@/components/ui/toast"
import { TooltipProvider } from "@/components/ui/tooltip"
import { routeTree } from "./routeTree.gen"
import { AppRoot } from "./AppRoot"

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,
      retry: 1,
    },
  },
})

const router = createRouter({ routeTree })

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}

export default function MainApp() {
  return (
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <Toaster>
          <AppRoot router={router} />
        </Toaster>
      </TooltipProvider>
    </QueryClientProvider>
  )
}
