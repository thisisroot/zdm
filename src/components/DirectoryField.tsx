import { api } from '../lib/api'

interface DirectoryFieldProps {
  label: string
  value: string
  onChange: (value: string) => void
}

export function DirectoryField({ label, value, onChange }: DirectoryFieldProps) {
  async function browse() {
    const picked = await api.chooseDirectory()
    if (picked) onChange(picked)
  }

  return (
    <div className="field">
      <label>{label}</label>
      <div className="with-btn">
        <input type="text" value={value} onChange={(e) => onChange(e.target.value)} />
        <button type="button" className="btn btn-sm" onClick={browse}>Browse…</button>
      </div>
    </div>
  )
}
