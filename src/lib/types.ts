export type DownloadStatus = 'queued' | 'downloading' | 'paused' | 'completed' | 'failed' | 'canceled'

export interface ActiveChunk {
  start: number
  end: number
  bytesDone: number
}

export interface DownloadRecord {
  id: string
  seq: number
  url: string
  name: string
  destination: string
  category: string
  queue: string
  connections: number
  status: DownloadStatus
  downloaded: number
  totalSize: number | null
  speedBps: number
  error: string | null
  activeChunks: ActiveChunk[]
}

export type Filter =
  | { kind: 'all' }
  | { kind: 'status'; status: DownloadStatus }
  | { kind: 'queue'; queueId: string }
  | { kind: 'category'; categoryId: string }

export function matchesFilter(record: DownloadRecord, filter: Filter): boolean {
  switch (filter.kind) {
    case 'all':
      return true
    case 'status':
      return record.status === filter.status
    case 'queue':
      return record.queue === filter.queueId
    case 'category':
      return record.category === filter.categoryId
  }
}

export interface QueueInfo {
  id: string
  name: string
}

export interface Settings {
  maxSimultaneousDownloads: number
  defaultConnections: number
  notifyOnCompletion: boolean
  categoryDirs: Record<string, string>
  defaultDir: string
  accentColor: string
}
