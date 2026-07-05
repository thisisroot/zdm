import { invoke } from '@tauri-apps/api/core'
import type { DownloadRecord, QueueInfo, Settings } from './types'

// Thin, typed wrapper around the Tauri commands defined in src-tauri/src/commands.rs.
// Argument keys are camelCase — Tauri maps them to the snake_case Rust parameter names.
export const api = {
  listDownloads: () => invoke<DownloadRecord[]>('list_downloads'),
  listQueues: () => invoke<QueueInfo[]>('list_queues'),
  getSettings: () => invoke<Settings>('get_settings'),
  updateSettings: (settings: Settings) => invoke<void>('update_settings', { settings }),
  chooseDirectory: () => invoke<string | null>('choose_directory'),

  addDownload: (args: { url: string; destinationDir: string; connections: number; category: string; queue: string; filename?: string }) =>
    invoke<string>('add_download', args),

  checkConflict: (destinationDir: string, url: string, filename?: string) =>
    invoke<string | null>('check_conflict', { destinationDir, url, filename }),

  addBatch: (args: {
    urlPattern: string
    destinationDir: string
    connections: number
    category: string
    queueName: string
  }) => invoke<string[]>('add_batch', args),

  addBatchUrls: (args: {
    urls: string[]
    destinationDir: string
    connections: number
    category: string
    queueName: string
  }) => invoke<string[]>('add_batch_urls', args),

  probeUrls: (urls: string[]) => invoke<UrlProbe[]>('probe_urls', { urls }),

  pauseDownload: (id: string) => invoke<void>('pause_download', { id }),
  resumeDownload: (id: string) => invoke<void>('resume_download', { id }),
  retryDownload: (id: string) => invoke<void>('retry_download', { id }),
  cancelDownload: (id: string) => invoke<void>('cancel_download', { id }),
  removeDownload: (id: string, deleteFile: boolean) => invoke<void>('remove_download', { id, deleteFile }),
  deleteQueue: (id: string) => invoke<void>('delete_queue', { id }),
  toggleQueue: (queueId: string) => invoke<void>('toggle_queue', { queueId }),
  toggleAll: () => invoke<void>('toggle_all'),
  reorderDownloads: (ids: string[]) => invoke<void>('reorder_downloads', { ids }),
}

export interface UrlProbe {
  url: string
  totalSize: number | null
  error: string | null
}
