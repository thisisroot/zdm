import { categoryById, formatBytes, formatEta, formatSpeed } from '../lib/categories'
import type { DownloadRecord, SpeedUnit } from '../lib/types'
import { SegmentBar } from './SegmentBar'
import { CancelIcon, FolderIcon, PauseIcon, PlayIcon, RetryIcon } from './icons'

interface DownloadRowProps {
  record: DownloadRecord
  selected: boolean
  checked: boolean
  queueName: string | null
  speedUnit: SpeedUnit
  onSelect: () => void
  onToggleCheck: () => void
  onPause: () => void
  onResume: () => void
  onRetry: () => void
  onRemove: () => void
  onOpenFolder: () => void
  onOpenFile: () => void
}

export function DownloadRow({
  record,
  selected,
  checked,
  queueName,
  speedUnit,
  onSelect,
  onToggleCheck,
  onPause,
  onResume,
  onRetry,
  onRemove,
  onOpenFolder,
  onOpenFile,
}: DownloadRowProps) {
  const cat = categoryById(record.category)
  const connectionsLabel =
    record.status === 'failed' ? 'connection reset · retry available' : `${record.connections} connections · ${pct(record)}%`

  return (
    <div
      className={`row${selected ? ' selected' : ''}${checked ? ' checked' : ''}`}
      onClick={onSelect}
      onDoubleClick={() => record.status === 'completed' && onOpenFile()}
    >
      <div className="row-top">
        <input
          type="checkbox"
          className="row-check"
          checked={checked}
          onClick={(e) => e.stopPropagation()}
          onChange={onToggleCheck}
          aria-label={`Select ${record.name}`}
        />
        <div className="filetype" style={{ background: cat.color }}>{cat.glyph}</div>
        <div className="row-name">
          <div className="fname">{record.name}</div>
          <div className="fmeta">
            <span>{hostOf(record.url)} · {formatBytes(record.totalSize)}</span>
            {queueName && queueName !== 'Default Queue' && <span className="queue-tag">{queueName}</span>}
          </div>
        </div>
        <span className={`status-pill ${record.status}`}>{record.status}</span>
        <div className="row-stats">
          <div className="speed tabular">{record.status === 'downloading' ? formatSpeed(record.speedBps, speedUnit) : ' '}</div>
          <div className="eta tabular">{formatEta(record)}</div>
        </div>
      </div>

      <SegmentBar record={record} />

      <div className="row-foot">
        <span className="segcount">{connectionsLabel}</span>
        <div className="row-actions" onClick={(e) => e.stopPropagation()}>
          {record.status === 'downloading' && <button title="Pause" onClick={onPause}><PauseIcon /></button>}
          {record.status === 'paused' && <button title="Resume" onClick={onResume}><PlayIcon /></button>}
          {(record.status === 'failed' || record.status === 'canceled') && (
            <button title="Retry" onClick={onRetry}><RetryIcon /></button>
          )}
          {record.status === 'completed' && <button title="Show in folder" onClick={onOpenFolder}><FolderIcon /></button>}
          <button title="Remove" onClick={onRemove}><CancelIcon /></button>
        </div>
      </div>
    </div>
  )
}

function pct(record: DownloadRecord): number {
  if (!record.totalSize) return 0
  return Math.round((record.downloaded / record.totalSize) * 100)
}

function hostOf(url: string): string {
  try {
    return new URL(url).host
  } catch {
    return url
  }
}
