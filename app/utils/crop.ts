import type { CropRect } from '~/types'

export type CropRegion = 'gameplay' | 'facecam' | 'chat'
export type CropHandle = 'move' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w' | 'nw'
export const cropRegions = [
  { key: 'gameplay', label: 'Spiel / Hauptbild', color: '#6b9fff' },
  { key: 'facecam', label: 'Facecam', color: '#b8a1ff' },
  { key: 'chat', label: 'Chat', color: '#38d6bd' },
] as const

// Work in source-image coordinates, independent of the displayed preview size.
export function transformCrop(rect: CropRect, handle: CropHandle, dx: number, dy: number): CropRect {
  if (!Number.isFinite(dx) || !Number.isFinite(dy) || (dx === 0 && dy === 0)) return { ...rect }
  const clamp = (value: number, min: number, max: number) => Math.max(min, Math.min(max, value))
  if (handle === 'move') return { ...rect, x: clamp(rect.x + dx, 0, 1 - rect.width), y: clamp(rect.y + dy, 0, 1 - rect.height) }
  const min = .02
  let left = rect.x, top = rect.y, right = rect.x + rect.width, bottom = rect.y + rect.height
  if (handle.includes('w')) left = clamp(left + dx, 0, right - min)
  if (handle.includes('e')) right = clamp(right + dx, left + min, 1)
  if (handle.includes('n')) top = clamp(top + dy, 0, bottom - min)
  if (handle.includes('s')) bottom = clamp(bottom + dy, top + min, 1)
  return { ...rect, x: left, y: top, width: Math.max(min, right - left), height: Math.max(min, bottom - top) }
}
