import { CATEGORIES } from '../lib/categories'
import type { DownloadRecord, DownloadStatus, Filter, QueueInfo } from '../lib/types'
import { matchesFilter } from '../lib/types'
import { CancelIcon, CategoryIcons, ListIcon, PauseIcon, PlayIcon } from './icons'

interface RailProps {
  downloads: DownloadRecord[]
  queues: QueueInfo[]
  filter: Filter
  onFilterChange: (filter: Filter) => void
  onToggleQueue: (queueId: string) => void
  onDeleteQueue: (queueId: string) => void
}

const STATUS_ITEMS: { status: DownloadStatus; label: string; dot: string }[] = [
  { status: 'downloading', label: 'Downloading', dot: 'var(--progress)' },
  { status: 'paused', label: 'Paused', dot: 'var(--warning)' },
  { status: 'queued', label: 'Queued', dot: 'var(--text-faint)' },
  { status: 'completed', label: 'Completed', dot: 'var(--success)' },
  { status: 'failed', label: 'Failed', dot: 'var(--error)' },
]

function sameFilter(a: Filter, b: Filter): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

export function Rail({ downloads, queues, filter, onFilterChange, onToggleQueue, onDeleteQueue }: RailProps) {
  const countFor = (f: Filter) => downloads.filter((d) => matchesFilter(d, f)).length

  return (
    <nav className="rail">
      <h3>Status</h3>
      <button
        className={`rail-item${sameFilter(filter, { kind: 'all' }) ? ' active' : ''}`}
        onClick={() => onFilterChange({ kind: 'all' })}
      >
        <span className="dot" style={{ background: 'var(--text-faint)' }} />
        All Downloads
        <span className="count">{downloads.length}</span>
      </button>
      {STATUS_ITEMS.map((item) => {
        const f: Filter = { kind: 'status', status: item.status }
        return (
          <button key={item.status} className={`rail-item${sameFilter(filter, f) ? ' active' : ''}`} onClick={() => onFilterChange(f)}>
            <span className="dot" style={{ background: item.dot }} />
            {item.label}
            <span className="count">{countFor(f)}</span>
          </button>
        )
      })}

      <h3>Queues</h3>
      {queues.map((q) => {
        const f: Filter = { kind: 'queue', queueId: q.id }
        const items = downloads.filter((d) => matchesFilter(d, f))
        const anyRunning = items.some((d) => d.status === 'downloading')
        return (
          <button key={q.id} className={`rail-item${sameFilter(filter, f) ? ' active' : ''}`} onClick={() => onFilterChange(f)}>
            <ListIcon />
            <span className="qname">{q.name}</span>
            <span
              className="rail-hover-action"
              title={anyRunning ? 'Pause queue' : 'Resume queue'}
              onClick={(e) => {
                e.stopPropagation()
                onToggleQueue(q.id)
              }}
            >
              {anyRunning ? <PauseIcon strokeWidth={2.4} /> : <PlayIcon strokeWidth={2.4} />}
            </span>
            {q.id !== 'default' && (
              <span
                className="rail-hover-action rail-hover-danger"
                title="Delete queue"
                onClick={(e) => {
                  e.stopPropagation()
                  if (window.confirm(`Delete "${q.name}"? Its downloads will move to the Default Queue.`)) {
                    onDeleteQueue(q.id)
                  }
                }}
              >
                <CancelIcon strokeWidth={2.4} />
              </span>
            )}
            <span className="count">{items.length}</span>
          </button>
        )
      })}

      <h3>Categories</h3>
      {CATEGORIES.map((cat) => {
        const f: Filter = { kind: 'category', categoryId: cat.id }
        const Icon = CategoryIcons[cat.id]
        return (
          <button key={cat.id} className={`rail-item${sameFilter(filter, f) ? ' active' : ''}`} onClick={() => onFilterChange(f)}>
            <Icon />
            {cat.label}
            <span className="count">{countFor(f)}</span>
          </button>
        )
      })}
    </nav>
  )
}
