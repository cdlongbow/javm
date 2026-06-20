/**
 * 番号 → DMM 官方封面 URL（零爬取直拼，与后端 `media/dmm.rs::designation_to_cid` 同规则）。
 *
 * cid = 字母前缀(小写) + 尾部数字补零到 5 位；拼 `pics.dmm.co.jp` 的 `pl.jpg`。
 * 仅覆盖有码主流（FANZA）；无码/FC2/素人等非 DMM 番号会 404（由 <img> 的 @error 兜底隐藏）。
 * 直接用作 <img src> 即可，浏览器/WebView 会按 CDN 缓存头缓存，重开不重复下载。
 */
export function dmmCoverUrl(code?: string | null): string | null {
    if (!code) return null
    const compact = code.replace(/\s+/g, '')
    // 尾部连续数字作 number，前面部分作 label
    const m = compact.match(/^(.*?)(\d+)$/)
    if (!m) return null
    const label = m[1].replace(/[^a-zA-Z0-9]/g, '').toLowerCase()
    if (!label) return null
    const num = parseInt(m[2], 10)
    if (Number.isNaN(num)) return null
    const cid = `${label}${String(num).padStart(5, '0')}`
    return `https://pics.dmm.co.jp/digital/video/${cid}/${cid}pl.jpg`
}
