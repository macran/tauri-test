import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { StartupStatus } from "./startup.types";

export const startupStatusQueryKey = ["startup-status"] as const;
export const pendingBackupsQueryKey = ["pending-backups"] as const;

/**
 * 启动阶段只需要短轮询；ready/failed 后不再重复请求。
 * Rust 端把业务错误作为 StartupStatus 返回，因此这里不依赖异常来表达启动失败。
 */
export function useStartupStatus() {
  return useQuery({
    queryKey: startupStatusQueryKey,
    queryFn: () => invoke<StartupStatus>("get_startup_status"),
    staleTime: Infinity,
    retry: 1,
    refetchInterval: (query) =>
      query.state.data?.phase === "starting" ? 250 : false,
  });
}

export function usePendingBackups(enabled = true) {
  return useQuery({
    queryKey: pendingBackupsQueryKey,
    queryFn: () => invoke<string[]>("get_pending_backups"),
    enabled,
    staleTime: 5_000,
    refetchInterval: 30_000,
    retry: 1,
  });
}

export function useRetryBackup() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: () => invoke<string[]>("retry_backup_now"),
    onSuccess: (remainingFiles) => {
      queryClient.setQueryData(pendingBackupsQueryKey, remainingFiles);
    },
  });
}
