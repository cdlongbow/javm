# 对标分析:JAV-Preview

- **仓库**:https://github.com/dbghelp/JAV-Preview
- **分析日期**:2026-06-17
- **分析维度**:在线预览(预告片 / 海报 / 截图)、DMM 资源获取
- **规模**:**Tauri + TypeScript(与本项目同栈)**,~8 星,无数据库桌面应用
- **定位**:搜索 + 查看 JAV 预览(海报/截图/预告片),绕过地理封锁的 DMM/FANZA

## 一、核心洞察:DMM 预览资源「零爬取」URL 拼接

`src/App.tsx`(逻辑全在单文件)——**不爬 DMM 页面/API**,而是 番号 → DMM cid → 直接拼官方 CDN 资源 URL:

| 资源 | URL 模板 |
|---|---|
| 海报(digital) | `https://pics.dmm.co.jp/digital/video/{cid}/{cid}pl.jpg` |
| 海报(mono) | `https://pics.dmm.co.jp/mono/movie/adult/{cid}/{cid}pl.jpg` |
| 截图 | `https://pics.dmm.co.jp/digital/video/{cid}/{cid}jp-{n}.jpg`(n=1,2,3…) |
| 预告片(digital) | `https://cc3001.dmm.co.jp/litevideo/freepv/{c1}/{c3}/{cid}/{cid}_dmb_w.mp4` |
| 预告片(mono) | `https://cc3001.dmm.co.jp/litevideo/freepv/{c1}/{c3}/{cid}/{cid}_mhb_w.mp4` |

- `cid` = 番号规范化(小写 + 补零,如 SSIS-001 → `ssis00001`)+ **前缀映射表**(代码 line 28:`siro→h_128` 等特例)。
- `c1`/`c3` = cid 首字母 / 前 3 字母(freepv 路径分层)。
- 预告片是 **mp4**(非 m3u8);清晰度后缀 `_dmb_w`/`_mhb_w`(还有 sm/dm 等档,需 fallback 探测)。

**「绕过地理封锁」的真相**:被封的是 DMM 网页/API,但 `pics.dmm.co.jp` 与 `cc3001.dmm.co.jp` 两个 **CDN 不做地理封锁** → 跳过页面、直连 CDN 拼 URL 即得官方海报/截图/预告片。Rust 侧仅 ~653 字节(Tauri 壳 + HTTP),核心在前端。

## 二、可借鉴提升点

| # | 提升点 | 它的做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **DMM 官方预告片(零爬取在线预览)** | 番号→cid→拼 `cc3001/freepv` mp4 直接播放 | 探索预览靠爬 MissAV/Jable 的 m3u8(CF/反爬/混淆) | **作为探索模块「在线预览」主源**:DMM 官方 trailer 直拼 URL,稳定、官方、无反爬;MissAV/Jable 作补充 | 🔴 高 |
| 2 | **DMM 官方海报/截图(零爬取)** | `pics.dmm.co.jp` 拼 URL | 刮削图片靠各源爬 | 作高清图片源补充(海报/截图),直拼 URL | 🟡 中 |
| 3 | **跳页面直连 CDN 思路** | 不碰被封页面,直连不封锁的 CDN | — | 通用技巧:能直拼 CDN 的资源就别爬页面 | 🟡 中 |
| 4 | **番号→DMM cid 转换 + 前缀映射** | paddedCode + 前缀映射表 + 路径分层 | 无 | 实现转换规则(覆盖有码主流);维护特例映射 | 🟡 中 |

## 三、局限

- **仅覆盖 DMM 体系**(有码主流 / FANZA);无码、FC2、素人、欧美、国产无此资源。
- 番号→cid 有例外(前缀映射、digital vs mono、补零位数),需维护规则。
- 清晰度后缀 `_dmb_w`/`_mhb_w`/… 因片而异,需 fallback 探测可用项。
- URL 规律依赖 DMM CDN 结构(多年稳定,但仍属非官方约定)。

## 四、关键结论

1. **同栈(Tauri/TS),最易借鉴。** 这是「在线预览」的理想主源:**DMM 官方预告片直拼 URL**,比爬 MissAV/Jable 的 m3u8 稳得多(官方 CDN、无地理封锁、无反爬、无 JS 混淆)。
2. **直接强化 `plans/探索模块-全网发现方案.md` 的「在线预览」**:预览主源改为「DMM 官方 trailer(直拼)优先 → MissAV/Jable m3u8 兜底」;海报/截图也可用 DMM CDN 作高清图片源。
3. 局限是只覆盖 DMM 有码主流,需与现有多源 + 无码源互补。

## 参考来源

- [JAV-Preview 仓库](https://github.com/dbghelp/JAV-Preview)
- 关联:`plans/探索模块-全网发现方案.md`、`plans/视频获取-快路径与并发优化方案.md`
