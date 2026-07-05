import { Sparkline } from './Sparkline'
import { PauseIcon, PlayIcon, PlusIcon, SearchIcon, SettingsIcon, ThemeIcon } from './icons'

interface TopBarProps {
  totalSpeedBps: number
  speedHistory: number[]
  activeCount: number
  activeConnections: number
  pausedCount: number
  search: string
  onSearchChange: (value: string) => void
  onToggleTheme: () => void
  onToggleAllActive: () => void
  onOpenSettings: () => void
  onOpenAdd: () => void
}

export function TopBar({
  totalSpeedBps,
  speedHistory,
  activeCount,
  activeConnections,
  pausedCount,
  search,
  onSearchChange,
  onToggleTheme,
  onToggleAllActive,
  onOpenSettings,
  onOpenAdd,
}: TopBarProps) {
  return (
    <div className="topbar">
      <div className="hero-speed">
        <span className="figure tabular">{(totalSpeedBps / 1e6).toFixed(1)}</span>
        <span className="unit">MB/s</span>
      </div>
      <Sparkline className="spark" data={speedHistory} width={240} height={68} />
      <div className="hero-meta">
        <span className="label">Active</span>
        <span className="value tabular">
          {activeCount} downloads · {activeConnections} connections
        </span>
      </div>
      <div style={{ flex: 1 }} />
      <div className="search">
        <SearchIcon />
        <input type="text" placeholder="Filter downloads…" value={search} onChange={(e) => onSearchChange(e.target.value)} />
      </div>
      <button className="btn btn-icon" title="Toggle theme" aria-label="Toggle theme" onClick={onToggleTheme}>
        <ThemeIcon />
      </button>
      {(activeCount > 0 || pausedCount > 0) && (
        <button
          className="btn btn-icon"
          title={activeCount > 0 ? 'Pause all active downloads' : 'Resume all paused downloads'}
          aria-label={activeCount > 0 ? 'Pause all active downloads' : 'Resume all paused downloads'}
          onClick={onToggleAllActive}
        >
          {activeCount > 0 ? <PauseIcon /> : <PlayIcon />}
        </button>
      )}
      <button className="btn btn-icon" title="Settings" aria-label="Settings" onClick={onOpenSettings}>
        <SettingsIcon />
      </button>
      <button className="btn btn-primary" onClick={onOpenAdd}>
        <PlusIcon />
        Add Download
      </button>
    </div>
  )
}
