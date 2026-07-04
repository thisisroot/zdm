import { useEffect, useState } from 'react'

interface ConflictModalProps {
  open: boolean
  fileName: string
  error: string | null
  onCancel: () => void
  onReplace: () => void
  onRename: (newName: string) => void
}

export function ConflictModal({ open, fileName, error, onCancel, onReplace, onRename }: ConflictModalProps) {
  const [renameTo, setRenameTo] = useState('')

  useEffect(() => {
    if (open) setRenameTo(suggestAlternateName(fileName))
  }, [open, fileName])

  if (!open) return null

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <div className="modal" style={{ width: 420 }}>
        <h2>File already exists</h2>
        <div className="sub">
          <b style={{ color: 'var(--text)' }}>{fileName}</b> already exists in this folder. Replace it, or save this download under a different name.
        </div>
        <div className="modal-body">
          <div className="field">
            <label>Save as</label>
            <input type="text" value={renameTo} onChange={(e) => setRenameTo(e.target.value)} autoFocus />
          </div>
        </div>
        <div className="modal-actions">
          {error && <span className="modal-error">{error}</span>}
          <button className="btn" onClick={onCancel}>Cancel</button>
          <button className="btn" onClick={() => onRename(renameTo.trim())} disabled={!renameTo.trim()}>
            Save as new file
          </button>
          <button className="btn btn-primary" onClick={onReplace}>Replace existing</button>
        </div>
      </div>
    </div>
  )
}

function suggestAlternateName(name: string): string {
  const dot = name.lastIndexOf('.')
  const stem = dot > 0 ? name.slice(0, dot) : name
  const ext = dot > 0 ? name.slice(dot) : ''
  return `${stem} (1)${ext}`
}
