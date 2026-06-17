# 对标分析:JableTV-MissAV-Downloader-GUI

- **仓库**:https://github.com/Alos21750/JableTV-MissAV-Downloader-GUI-2026
- **分析日期**:2026-06-17
- **分析维度**:在线视频站的视频链接 / 字幕 / 列表分类抓取
- **规模**:Python,~150 星,活跃维护
- **定位**:JableTV / MissAV / SupJav 在线视频**浏览 + 下载**器(CustomTkinter GUI)

## 一、视频链接(m3u8)获取

两站都先用 `curl_cffi`(优先)或 `cloudscraper` **模拟浏览器 TLS 指纹过 Cloudflare**,抓详情页 HTML。m3u8 拿法不同:

| 站点 | m3u8 位置 | 提取方式 | 难度 |
|---|---|---|---|
| **JableTV** | 明文写在页面源码 | 直接正则 `https://.+m3u8`;另配 `fs1.app` 镜像容错 | 低 |
| **MissAV** | JS packer 混淆 | `eval(function(p,a,c,k,e,d){...})`(Dean Edwards packer)→ `_unpack_js_eval` 手写解包(进制转换+词典替换)→ 还原后正则 `source='...m3u8'` | 高 |

下载:HLS 的 AES-128 加密 TS 分片 → 多线程下载 → 自动解密合并 MP4。

## 二、字幕获取 —— 硬字幕片源,非字幕文件

不下载独立 `.srt/.vtt`。"中文字幕"是**硬烧录在画面(hardsub)**,通过分类筛选中字版片源:
- MissAV:`missav.ai/dm278/chinese-subtitle`(URL 带 `-chinese-subtitle` 即中字版)
- JableTV:`jable.tv/categories/chinese-subtitle/`

`jable_smalltool` 的"中文字幕一键全抓" = 定时抓这两个分类的新片。

## 三、列表 / 分类 / tag 抓取

首页展示**非本地视频,是实时爬站点列表页**(`M3U8Sites/SiteJableTV.py`、`SiteMissAV.py`):
- **JableTV**:爬列表页,BeautifulSoup 解析 `div#list_videos`,带排序(最高相關/最新/最多觀看)+ 分页(`totalPages`/`totalLinks`)。
- **MissAV**:内置固定分类表(今日/本周/本月热门、中文字幕、最近更新、新作、无码流出、SIRO、FC2、麻豆、东京热、一本道…),每分类一个 `dmXXX/xxx` URL;也支持 `fetch_categories` 动态抓。
- **切换分类/tag = 换 URL 重爬列表页**;翻页 = URL 加页码;语言 = cookie(JableTV `kt_rt_lang`)或 URL 语言段(`/cn/`、`/en/`)。

## 四、对本项目的意义与可借鉴点

这是「在线片源浏览 + 下载」维度,与本项目「本地文件管理」不同,但与 JavHub(在线播放)、CodeSeek(在线搜索)同属"给应用加在线片源入口"。前置条件本项目已具备:已有 N_m3u8DL-RE(m3u8 下载器)+ 反爬 fetcher(wreq + TLS 指纹)。

| # | 提升点 | 它的做法 | 建议 | 优先级 |
|---|---|---|---|---|
| 1 | 在线片源浏览 + 下载入口 | 爬站点列表页/分类,选片下载 m3u8 | 评估是否给本地管理工具加「在线浏览/下载」模块 | 🟡 中(产品方向) |
| 2 | m3u8 解析(过 CF + 解混淆) | curl_cffi 过 CF;MissAV 解 JS packer | 接在线站时复用思路;wreq 已能做 TLS 指纹 | 🟡 中 |
| 3 | HLS 多线程下载 + 合并 | AES-128 解密 + TS 合并 MP4 | 已有 N_m3u8DL-RE 可直接做 | 🟢 低 |
| 4 | 镜像容错 | 多镜像域名 fetch_with_mirrors | 反爬源加镜像兜底 | 🟢 低 |

⚠️ 站点强依赖、易改版(尤其 MissAV 的 JS 混淆随时变);合规上属在线抓取下载,定位个人使用。

## 参考来源

- [JableTV-MissAV-Downloader-GUI 仓库](https://github.com/Alos21750/JableTV-MissAV-Downloader-GUI-2026)
