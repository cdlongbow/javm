import { describe, it, expect } from 'vitest'
import { pickRealLink } from '../pickRealLink'
import type { VideoLink } from '@/lib/tauri'

const mk = (p: Partial<VideoLink>): VideoLink => ({
  url: p.url ?? 'u',
  linkType: p.linkType ?? 'm3u8',
  isHls: p.isHls ?? true,
  resolution: p.resolution ?? null,
  ...p,
})

describe('pickRealLink', () => {
  it('无任何带时长链接 → null', () => {
    expect(pickRealLink([mk({ url: 'a' })])).toBeNull()
  })

  it('最长时长 < 300 秒（全是广告/片段）→ null', () => {
    expect(
      pickRealLink([
        mk({ url: 'a', durationSecs: 100 }),
        mk({ url: 'b', durationSecs: 250 }),
      ]),
    ).toBeNull()
  })

  it('时长 ≥ 300 秒 → 选出正片', () => {
    const r = pickRealLink([
      mk({ url: 'ad', durationSecs: 60 }),
      mk({ url: 'main', durationSecs: 3600 }),
    ])
    expect(r?.url).toBe('main')
  })

  it('同片多清晰度（时长接近）→ 优先 master', () => {
    const r = pickRealLink([
      mk({ url: 'v1080', durationSecs: 3600, height: 1080, isMaster: false }),
      mk({ url: 'master', durationSecs: 3600, height: 0, isMaster: true }),
    ])
    expect(r?.url).toBe('master')
  })

  it('都非 master → 选更高分辨率', () => {
    const r = pickRealLink([
      mk({ url: 'v720', durationSecs: 3600, height: 720 }),
      mk({ url: 'v1080', durationSecs: 3600, height: 1080 }),
    ])
    expect(r?.url).toBe('v1080')
  })
})
