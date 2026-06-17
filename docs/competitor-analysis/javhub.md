# 对标分析:JavHub

- **仓库**:https://github.com/Kongmei-ovo/JavHub
- **分析日期**:2026-06-17
- **分析维度**:刮削、NFO、番号命名规范(及使用侧功能)

## 一、定位差异

JavHub 与本项目定位差异很大:它是**自托管 Web 服务**,主打网盘在线播放、媒体服务器兼容、订阅通知;刮削同样**外包给配套的 JavInfoApi**(与 MetaTube 思路一致),自己不做番号识别。因此在"刮削/NFO/番号命名"维度几乎无可借鉴点,其价值在**使用侧功能**(播放、订阅、网盘)。

| 维度 | 我们 (JAVManager) | JavHub |
|---|---|---|
| 形态 | 单机桌面应用 (Tauri) | 自托管 Web 服务 (Docker 部署,多端访问) |
| 前端 | Vue 3 + Vite | Vue 3 + Vite |
| 后端 | Rust | Python 3.11 + FastAPI |
| 存储 | SQLite (rusqlite) | PostgreSQL + Redis |
| 刮削 | 自研 16 源 HTML 解析 | 外包给配套 JavInfoApi |
| 资源 | 本地文件管理 | 网盘(115 Open)在线播放 + 媒体服务器 |
| 部署 | 桌面安装包 | Docker / Nginx / Cloudflare |

## 二、可借鉴的提升点

| # | 提升点 | JavHub 的做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **观看进度 / 继续观看** | resume + continue watching,记录已看 | 无播放进度记录 | videos 表增 `last_played_at`/`position`,首页"继续观看"列表 | 🔴 高 |
| 2 | **影片元数据独立于文件** | "movies exist independently of downloaded files",可管理未下载影片 | 仅管理已扫描的本地文件 | 支持"未拥有/愿望单"影片元数据收藏,后续补本地文件时自动关联 | 🟡 中 |
| 3 | **订阅 + 定时刷新** | 演员/系列订阅 + scheduled refresh tasks | 无订阅机制 | 演员/系列订阅,后台定时检查新作并提醒 | 🟡 中 |
| 4 | **网盘 + 在线播放** | 115 Open 绑定 + 在线播放资源 | 仅本地文件(`.strm` 已在 JvedioNext 分析中提) | 与 `.strm` 远程串流合并规划,支持网盘/远程资源 | 🟡 中 |
| 5 | **任务完成通知** | Telegram bot 推送 | 无通知 | 用**桌面系统通知**(刮削/下载完成),不引入 Telegram | 🟢 低 |
| 6 | 作为媒体服务器 (Emby 兼容 API) | movie-focused Emby/Infuse/VidHub/SenPlayer API | 生成 NFO 供媒体库读取 | **不建议** —— 定位不符,继续走 NFO 路线即可 | ⚪ 不建议 |

## 三、关键结论

1. **刮削/番号维度无新增可借鉴** —— JavHub 和 JvedioNext 一样把刮削外包给配套统一 API,我们自研 16 源在这一维度仍是优势,不需改动。
2. **真正的启发在"使用侧"** —— 观看进度(#1)、订阅(#3)是桌面工具也能直接受益、且与定位契合的功能,建议优先。
3. **架构不建议借鉴** —— JavHub 的 Web 服务 / Docker / 多端访问是另一条产品路线(适合 NAS 多设备),与我们单机桌面定位冲突,短期不应跟进。

## 参考来源

- [JavHub 仓库](https://github.com/Kongmei-ovo/JavHub)
