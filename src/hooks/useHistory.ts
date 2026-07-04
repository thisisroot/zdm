import { useEffect, useRef, useState } from 'react'

/** Rolling sample window for sparklines — reads `getValue()` on a fixed tick
 * rather than reacting to every prop change, so a burst of progress events
 * doesn't distort the time axis. */
export function useHistory(getValue: () => number, length = 48, intervalMs = 500): number[] {
  const [history, setHistory] = useState<number[]>(() => new Array(length).fill(0))
  const getValueRef = useRef(getValue)
  getValueRef.current = getValue

  useEffect(() => {
    const id = setInterval(() => {
      setHistory((prev) => [...prev.slice(1), getValueRef.current()])
    }, intervalMs)
    return () => clearInterval(id)
  }, [length, intervalMs])

  return history
}
