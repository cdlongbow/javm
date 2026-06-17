# 对标分析:NASSAV

- **仓库**:https://github.com/Satoing/NASSAV
- **分析日期**:2026-06-17
- **分析维度**:在线视频源 m3u8 获取、可插拔下载器架构
- **规模**:Python(下载核心)+ Go(HTTP API)+ Vue(前端),~376 星(本批最高)
- **定位**:NAS 全自动下载器——从在线站下载 m3u8 → 整理 → JavBus 刮削 + NFO → 入媒体库(Emby/Jellyfin),Docker/cron 自动化

## 一、MissAV m3u8 获取(重点,比此前 downloader 更优)

`src/downloader/missAVDownloader.py` 不完整解 JS packer,而是走 CDN 固定地址:
1. 正则抠 UUID 碎片:`re.search(r"m3u8\|([a-f0-9\|]+)\|com\|surrit\|https\|video", html)`,把 `|` 分隔片段**反转重组**:`"-".join(group.split("|")[::-1])`。
2. 拼固定 CDN:`https://surrit.com/{uuid}/playlist.m3u8`(MissAV 视频统一托管在 surrit.com)。
3. 请求 master m3u8,解析 `BANDWIDTH`/`RESOLUTION`,`_get_highest_quality_m3u8` **选最高清晰度**子流。

→ 对比 `jabletv-missav-downloader.md` 的"完整解包 packer":本法**更轻巧**,免还原整段 packer,只需抠 UUID。

**MissAV URL 规律**(getHTML 依次尝试,顺带区分类型):
- `/cn/{avid}-chinese-subtitle`(中文字幕)
- `/cn/{avid}-uncensored-leak`(无码流出)
- `/cn/{avid}`(普通)
- `/dm13/cn/{avid}`(备用)

## 二、可插拔下载器架构

`src/downloader/downloaderBase.py`:`Downloader(ABC)` 抽象基类,新增源只实现 3 个方法:
- `getDownloaderName() -> str`
- `getHTML(avid) -> Optional[str]`
- `parseHTML(html, avid) -> Optional[AVDownloadInfo]`(只需拿到 m3u8)

基类提供通用 `downloadInfo()`(编排)+ `downloadM3u8()`(下载)。**关键设计:各下载器只管拿视频流,元数据统一走 MissAV/JavBus**(职责分离)。已实现源:MissAV / Jable / HohoJ / Memo / KanAV,带权重优先级。

## 三、其余能力
- JavBus 刮削 + NFO 生成(Emby/Jellyfin 兼容);番号识别(车牌号)。
- SQLite 去重 + 队列 + 文件锁(单一下载任务)。
- Go HTTP API 远程控制 + Vue 预览页;cron 定时批量;Docker 部署。
- 数据源标注:MissAV(全、反爬少、720-1080p)/ Jable(中字多、1080p、反爬严)/ HohoJ(高清、基本无反爬)/ Memo(更新及时)。

## 四、可借鉴提升点

| # | 提升点 | NASSAV 做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **MissAV m3u8 轻量解法** | surrit.com CDN 固定地址 + UUID 碎片反转,免完整解包 | 优化方案里 MissAV 写的是"解 JS packer" | **替换方案 3.1 的 MissAV 解法为此法**,快路径即可拿,免 WebView | 🔴 高 |
| 2 | **视频下载源插件化** | 基类 3 方法(name/getHTML/parseHTML),元数据统一 | video_finder 单一 WebView,无视频源插件抽象 | 把"快路径视频源"抽象成 trait(呼应快路径+WebView池方案) | 🟡 中 |
| 3 | **MissAV 多 URL 探测** | 字幕/无码/普通/备用依次试 | — | 快路径按此顺序探测,顺带得字幕/无码标记 | 🟡 中 |
| 4 | **master m3u8 选最高清晰度** | 解析 bandwidth/resolution 取最高 | 拦截到啥用啥 | 拿到 master 后按清晰度选(对应"画质选择") | 🟡 中 |
| 5 | NAS 自动化(cron/Docker/HTTP API) | 定时批量、远程控制 | 桌面应用 | 产品方向不同,参考 | 🟢 低 |
| 6 | **演员头像从 JavBus 直接抓** | `scraper.py` 详情页 `avatar-box` 正则取「名字+头像URL」(actress dict) | actors 表仅名字,曾以为头像要靠 GFriends/XsList | **正在爬的 JavBus 详情页就有头像**,顺手抓即可,成本极低 | 🔴 高 |
| 7 | **搜索式取流(HohoJ 型)** | 无番号直链的站:先 `search?text={code}` 取 id → `embed?id={id}`(带 Referer)正则 `videoSrc` | 快路径只设想直链站 | 快路径支持「搜索→id→embed」两步型站点 | 🟢 中低 |

## 五、各站快路径解法一览(二轮深挖)

各下载器的 m3u8 提取都只是站点专属的一两条正则,印证「快路径插件每站 20-30 行」工作量极小:

| 站点 | 取流方式 | 关键正则/地址 |
|---|---|---|
| **Jable** | 详情页明文 | `var hlsUrl = '(https?://[^']+)'` |
| **MissAV** | CDN 重组 | 抠 UUID 反转 → `https://surrit.com/{uuid}/playlist.m3u8` → master 选清晰度 |
| **HohoJ** | 搜索→embed 两步 | `search?text={code}` 取 `id=(\d+)` → `embed?id={id}`(带 Referer)→ `var videoSrc = "(...)"` |

**JavBus 刮削(`scraper.py`)正则**:title `<title>(.*?) - JavBus`、cover `a.bigImage href`、**actress `avatar-box…photo-frame…img src + span`(名字+头像)**、fanart `sample-box href .jpg`;去重用 `INSERT OR IGNORE`(UNIQUE 约束)。

## 六、关键结论

1. **第三次印证在线站抓取思路,并给出 MissAV 的最优解法**:surrit.com CDN + UUID 反转,直接强化 `docs/plans/视频获取-快路径与并发优化方案.md` 的快路径——**MissAV 不必完整解包、也未必要 WebView**。
2. **可插拔下载器架构**(基类 3 方法 + 元数据统一)是干净的视频源插件化范式,与我们的快路径/WebView 池方案天然契合。
3. **演员头像就在 JavBus 详情页**(`avatar-box` 的 img):我们已经在爬 JavBus,顺手即可补齐一直缺失的演员头像,无需引入 GFriends/XsList。本轮最高性价比提升点。
4. NASSAV 整体是"自动下载入库"工具,与本项目"本地管理"定位不同,但其**在线视频源解析层 + JavBus 刮削层**都是直接可借鉴的。

## 参考来源

- [NASSAV 仓库](https://github.com/Satoing/NASSAV)
- 关联:`docs/competitor-analysis/jabletv-missav-downloader.md`、`docs/plans/视频获取-快路径与并发优化方案.md`
