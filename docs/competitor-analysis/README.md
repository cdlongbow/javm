# 竞品 / 同类库对标分析

记录对同类视频资源管理工具的调研与对比,提炼可借鉴的提升点。

## 已分析

| 项目 | 技术栈 | 分析重点 | 文档 | 日期 |
|---|---|---|---|---|
| JvedioNext | Tauri + .NET 8 + MetaTube | 刮削 / NFO / 番号命名规范 | [jvedionext.md](./jvedionext.md) | 2026-06-17 |
| JavHub | FastAPI + Vue3 + PostgreSQL + Docker | 使用侧功能(播放/订阅/网盘);刮削外包 | [javhub.md](./javhub.md) | 2026-06-17 |
| mediamatrix-jav-expert | Python 插件 | 解析/番号识别;**字段级跨源融合** | [mediamatrix-jav-expert.md](./mediamatrix-jav-expert.md) | 2026-06-17 |
| PornBoss | Go + React + SQLite + mpv | 同类最接近品;字段级分工补全/有码无码分轨/作品-文件解耦 | [pornboss.md](./pornboss.md) | 2026-06-17 |
| CodeSeek | TS + Cloudflare Workers | **磁力链接获取机制**(爬 JavBus ajax 接口) | [codeseek.md](./codeseek.md) | 2026-06-17 |
| JableTV-MissAV-Downloader | Python + CustomTkinter | 在线站 m3u8 解析(JS 解混淆)/硬字幕分类/列表抓取 | [jabletv-missav-downloader.md](./jabletv-missav-downloader.md) | 2026-06-17 |
| hyperq/jav | **Rust** + ratatui + scraper | **磁力获取+排序(同栈可移植)**/有码无码 URL 前缀切换 | [hyperq-jav.md](./hyperq-jav.md) | 2026-06-17 |
| NASSAV | Python + Go + Vue | **MissAV m3u8 轻量解法(surrit CDN)**/可插拔下载器架构 | [nassav.md](./nassav.md) | 2026-06-17 |
| javspider_stack | Python + FastAPI 异步 | 反爬工具箱(成功率加权代理池/镜像轮换/限速退避) | [javspider-stack.md](./javspider-stack.md) | 2026-06-17 |
| JAV_MovieManager | C# / .NET + React | 演员维度深耕(gfriends 头像源/minnano-av 资料/男优刮削) | [jav-moviemanager.md](./jav-moviemanager.md) | 2026-06-17 |
| JAV-Preview | **Tauri + TS(同栈)** | **DMM 预告片/海报/截图零爬取(直拼 CDN URL)** | [jav-preview.md](./jav-preview.md) | 2026-06-17 |
| OpenAver | Python + FastAPI + PyWebView | 片商名规范化映射/跨语言标签别名/规则式相似探索 | [openaver.md](./openaver.md) | 2026-06-17 |

## 待分析

_(后续追加)_

## 约定

- 每个对标项目一个独立 markdown 文件,文件名用 kebab-case。
- 文档结构建议:定位差异 → 整体能力对比 → 可借鉴提升点(带优先级)→ 架构决策 → 落地顺序 → 参考来源。
- 新增分析后,在上方"已分析"表格登记一行。
