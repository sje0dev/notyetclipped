import { describe, expect, test } from 'bun:test'
import { transformCrop } from '../app/utils/crop'
import type { CropHandle } from '../app/utils/crop'

describe('source-image crop gestures', () => {
  const source = { x: .2, y: .3, width: .4, height: .2 }
  test('dragging beyond the image keeps the complete box visible and its size unchanged', () => {
    expect(transformCrop(source, 'move', -2, 2)).toEqual({ x: 0, y: .8, width: .4, height: .2 })
    expect(transformCrop(source, 'move', .1, -.1).x).toBeCloseTo(.3)
    expect(source).toEqual({ x: .2, y: .3, width: .4, height: .2 })
  })
  test('shrinking a top-left corner preserves the opposite corner and never flips the crop', () => {
    const result = transformCrop(source, 'nw', 10, 10)
    expect(result.width).toBeCloseTo(.02)
    expect(result.height).toBeCloseTo(.02)
    expect(result.x + result.width).toBeCloseTo(.6)
    expect(result.y + result.height).toBeCloseTo(.5)
  })
  test('all eight grips remain valid at image boundaries under large movements', () => {
    for (const handle of ['n', 'ne', 'e', 'se', 's', 'sw', 'w', 'nw'] as CropHandle[]) {
      for (const dx of [-3, -.01, 0, .01, 3]) for (const dy of [-3, -.01, 0, .01, 3]) {
        const result = transformCrop(source, handle, dx, dy)
        expect(result.x).toBeGreaterThanOrEqual(0)
        expect(result.y).toBeGreaterThanOrEqual(0)
        expect(result.width).toBeGreaterThanOrEqual(.02)
        expect(result.height).toBeGreaterThanOrEqual(.02)
        expect(result.x + result.width).toBeLessThanOrEqual(1)
        expect(result.y + result.height).toBeLessThanOrEqual(1)
      }
    }
  })
})
