/**
 * 番号 → DMM 官方封面 URL（零爬取直拼，与后端 `media/dmm.rs::designation_to_cid` 同规则）。
 *
 * cid = 字母前缀(小写) + 尾部数字补零到 5 位。封面分 digital / mono 两路（与后端 probe 一致：
 * digital 优先、回退 mono）。仅覆盖有码主流（FANZA）；无码/FC2/素人会 404（由 <img> @error 兜底）。
 * 直接用作 <img src>，浏览器/WebView 按 CDN 缓存头缓存，重开不重复下载。
 */
function dmmCid(code?: string | null): string | null {
    if (!code) return null
    const compact = code.replace(/\s+/g, '')
    // 尾部连续数字作 number，前面部分作 label
    const m = compact.match(/^(.*?)(\d+)$/)
    if (!m) return null
    const label = m[1].replace(/[^a-zA-Z0-9]/g, '').toLowerCase()
    if (!label) return null
    const num = parseInt(m[2], 10)
    if (Number.isNaN(num)) return null
    return `${label}${String(num).padStart(5, '0')}`
}

/** digital 路径封面（优先） */
export function dmmCoverUrl(code?: string | null): string | null {
    const cid = dmmCid(code)
    return cid ? `https://pics.dmm.co.jp/digital/video/${cid}/${cid}pl.jpg` : null
}

/** mono 路径封面（digital 没有时回退） */
export function dmmMonoCoverUrl(code?: string | null): string | null {
    const cid = dmmCid(code)
    return cid ? `https://pics.dmm.co.jp/mono/movie/adult/${cid}/${cid}pl.jpg` : null
}

/**
 * 识别 DMM「占位图」（封面不存在时 302 跳转到的固定图，浏览器跟随跳转后会显示出来）：
 * - digital → `now_printing.jpg` 590×800
 * - mono → noimage `adult_pl.jpg` 90×122
 *
 * 这两个尺寸是 DMM 占位图的固定特征，正常横版封面不会是这两个尺寸，故按尺寸精准命中即视为无封面。
 */
export function isDmmPlaceholderSize(w?: number | null, h?: number | null): boolean {
    if (!w || !h) return false
    return (w === 590 && h === 800) || (w === 90 && h === 122)
}

/** 是否 DMM 图片 URL（占位图识别只对 DMM 源生效，避免误伤本地/数据源真封面） */
export function isDmmImageUrl(src?: string | null): boolean {
    return !!src && /dmm\.(co\.jp|com)/i.test(src)
}
