import { useEffect, useRef } from 'react'

interface SparklineProps {
  data: number[]
  color?: string
  className?: string
  width?: number
  height?: number
}

export function Sparkline({ data, color = 'var(--progress)', className, width = 240, height = 68 }: SparklineProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return

    const w = canvas.width
    const h = canvas.height
    ctx.clearRect(0, 0, w, h)

    const max = Math.max(...data, 1)
    const step = w / Math.max(data.length - 1, 1)
    const resolved = resolveColor(color)

    ctx.strokeStyle = resolveColor('var(--border-soft)')
    ctx.lineWidth = 1
    for (let g = 1; g < 3; g++) {
      const gy = h - h * (g / 3)
      ctx.beginPath()
      ctx.moveTo(0, gy)
      ctx.lineTo(w, gy)
      ctx.stroke()
    }

    const yFor = (v: number) => h - (v / max) * h * 0.86 - 4

    ctx.beginPath()
    ctx.moveTo(0, yFor(data[0] ?? 0))
    data.forEach((v, i) => ctx.lineTo(i * step, yFor(v)))
    ctx.lineTo(w, h)
    ctx.lineTo(0, h)
    ctx.closePath()
    const gradient = ctx.createLinearGradient(0, 0, 0, h)
    gradient.addColorStop(0, colorMix(resolved, 0.3))
    gradient.addColorStop(1, colorMix(resolved, 0))
    ctx.fillStyle = gradient
    ctx.fill()

    ctx.beginPath()
    data.forEach((v, i) => (i === 0 ? ctx.moveTo(0, yFor(v)) : ctx.lineTo(i * step, yFor(v))))
    ctx.strokeStyle = resolved
    ctx.lineWidth = 1.6
    ctx.stroke()

    ctx.beginPath()
    ctx.arc(w - 2, yFor(data[data.length - 1] ?? 0), 2.6, 0, Math.PI * 2)
    ctx.fillStyle = resolved
    ctx.fill()
  }, [data, color])

  return <canvas ref={canvasRef} className={className} width={width} height={height} />
}

function resolveColor(value: string): string {
  if (value.startsWith('var(')) {
    const name = value.slice(4, -1)
    const resolved = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
    return resolved || '#4fb477'
  }
  return value
}

function colorMix(hex: string, alpha: number): string {
  const clean = hex.replace('#', '')
  if (clean.length !== 6) return `rgba(79,180,119,${alpha})`
  const r = parseInt(clean.slice(0, 2), 16)
  const g = parseInt(clean.slice(2, 4), 16)
  const b = parseInt(clean.slice(4, 6), 16)
  return `rgba(${r},${g},${b},${alpha})`
}
