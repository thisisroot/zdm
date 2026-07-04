import type { DownloadRecord } from './types'

export interface Category {
  id: string
  label: string
  glyph: string
  color: string
}

// Order doubles as display order in the rail and in modal chip lists.
export const CATEGORIES: Category[] = [
  { id: 'video', label: 'Video', glyph: 'VID', color: 'var(--accent)' },
  { id: 'audio', label: 'Audio', glyph: 'MP3', color: 'var(--plum)' },
  { id: 'archive', label: 'Compressed', glyph: 'ZIP', color: 'var(--warning)' },
  { id: 'docs', label: 'Documents', glyph: 'PDF', color: 'var(--text-faint)' },
  { id: 'disc', label: 'Disc Images', glyph: 'ISO', color: 'var(--teal)' },
  { id: 'software', label: 'Software', glyph: 'APP', color: 'var(--blue)' },
  { id: 'image', label: 'Images', glyph: 'IMG', color: 'var(--rose)' },
  { id: 'ebook', label: 'Ebooks', glyph: 'EPB', color: 'var(--success)' },
  { id: 'font', label: 'Fonts', glyph: 'TTF', color: 'var(--progress)' },
  { id: 'torrent', label: 'Torrents', glyph: 'TOR', color: 'var(--error)' },
  { id: 'other', label: 'Other', glyph: 'OTH', color: 'var(--text-dim)' },
]

const EXT_TO_CATEGORY: Record<string, string> = {
  iso: 'disc', img: 'disc', bin: 'disc', nrg: 'disc',
  mp4: 'video', mkv: 'video', avi: 'video', mov: 'video', webm: 'video', flv: 'video', wmv: 'video', m4v: 'video',
  mp3: 'audio', flac: 'audio', wav: 'audio', m4a: 'audio', ogg: 'audio', aac: 'audio', opus: 'audio', wma: 'audio',
  zip: 'archive', rar: 'archive', '7z': 'archive', tar: 'archive', gz: 'archive', xz: 'archive', bz2: 'archive',
  pdf: 'docs', doc: 'docs', docx: 'docs', txt: 'docs', md: 'docs', rtf: 'docs', odt: 'docs',
  xls: 'docs', xlsx: 'docs', ods: 'docs', ppt: 'docs', pptx: 'docs', odp: 'docs', csv: 'docs',
  exe: 'software', dmg: 'software', deb: 'software', rpm: 'software', appimage: 'software', msi: 'software', pkg: 'software',
  apk: 'software', appx: 'software',
  jpg: 'image', jpeg: 'image', png: 'image', gif: 'image', webp: 'image', svg: 'image', bmp: 'image', heic: 'image',
  epub: 'ebook', mobi: 'ebook', azw: 'ebook', azw3: 'ebook', fb2: 'ebook', cbz: 'ebook', cbr: 'ebook',
  ttf: 'font', otf: 'font', woff: 'font', woff2: 'font', eot: 'font',
  torrent: 'torrent',
}

export function detectCategory(name: string): string {
  const lower = name.toLowerCase()
  if (/\.(tar\.gz|tar\.xz|tar\.bz2)$/.test(lower)) return 'archive'
  const match = lower.match(/\.([a-z0-9]+)$/)
  if (!match) return 'other'
  return EXT_TO_CATEGORY[match[1]] ?? 'other'
}

export function categoryById(id: string): Category {
  return CATEGORIES.find((c) => c.id === id) ?? CATEGORIES.find((c) => c.id === 'other')!
}

/** Display/category-detection only — the backend independently derives and
 * sanitizes the real filename it writes to disk. */
export function filenameFromUrl(url: string): string {
  const segments = url.split('/').filter(Boolean)
  const last = segments[segments.length - 1] || 'download'
  // Strip query/fragment before anything else — leaving them in breaks the
  // trailing-extension match below (e.g. `file.rar?12345` has no extension
  // by that regex), which silently misdetects category on any URL with a
  // query string.
  const withoutQuery = last.split(/[?#]/)[0] || 'download'
  try {
    return decodeURIComponent(withoutQuery)
  } catch {
    return withoutQuery
  }
}

export function formatBytes(bytes: number | null): string {
  if (bytes == null) return '—'
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  return `${(bytes / 1e3).toFixed(0)} KB`
}

export function formatSpeed(bps: number): string {
  return `${(bps / 1e6).toFixed(1)} MB/s`
}

export function formatEta(record: DownloadRecord): string {
  if (record.status !== 'downloading' || record.speedBps <= 0 || record.totalSize == null) return '—'
  const remainingSeconds = (record.totalSize - record.downloaded) / record.speedBps
  const minutes = Math.floor(remainingSeconds / 60)
  const seconds = Math.floor(remainingSeconds % 60)
  return `${minutes}m ${seconds}s left`
}

/** Client-side preview only — the server independently parses and validates the pattern. */
export function parseBatchPatternPreview(pattern: string, limit = 500): string[] {
  const match = pattern.match(/\[(\d+)-(\d+)\]/)
  if (!match) return []
  const [, startStr, endStr] = match
  const start = parseInt(startStr, 10)
  const end = parseInt(endStr, 10)
  if (Number.isNaN(start) || Number.isNaN(end) || end < start || end - start > limit) return []
  const width = startStr.length
  const results: string[] = []
  for (let n = start; n <= end; n++) {
    results.push(pattern.replace(/\[(\d+)-(\d+)\]/, String(n).padStart(width, '0')))
  }
  return results
}
