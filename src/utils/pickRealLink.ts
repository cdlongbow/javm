import type { VideoLink } from '@/lib/tauri'

/**
 * 从一个源捕获到的链接里选出「真实正片」：
 * 时长最长（≥ 300 秒）的那一条；同片多清晰度时优先主列表(master)、再高分辨率、再长时长。
 * 与原 VideoLinkFinder.vue 的 realLink 逻辑保持一致。
 */
export function pickRealLink(links: VideoLink[]): VideoLink | null {
  const analyzed = links.filter((l) => (l.durationSecs ?? 0) > 0)
  if (!analyzed.length) return null

  const maxDur = Math.max(...analyzed.map((l) => l.durationSecs ?? 0))
  if (maxDur < 300) return null

  const candidates = analyzed.filter((l) => (l.durationSecs ?? 0) >= maxDur * 0.9)
  candidates.sort((a, b) => {
    if (!!b.isMaster !== !!a.isMaster) return (b.isMaster ? 1 : 0) - (a.isMaster ? 1 : 0)
    const hDiff = (b.height ?? 0) - (a.height ?? 0)
    if (hDiff !== 0) return hDiff
    return (b.durationSecs ?? 0) - (a.durationSecs ?? 0)
  })
  return candidates[0] ?? null
}
