# 对标分析:OpenAver

- **仓库**:https://github.com/slive777/OpenAver
- **分析日期**:2026-06-17
- **分析维度**:规范化映射、相似探索、标签别名
- **规模**:Python(FastAPI + PyWebView 桌面),~112 星,3400+ 测试,成熟
- **定位**:JAV 刮削 + NFO/封面生成(Jellyfin/Emby/Kodi),8 内置源 + 可选 MetaTube 联邦

## 一、定位

与本项目同路线(本地刮削 + NFO + 多源 + 可选 MetaTube),许多能力我们已规划(演员头像、有码无码、字段合并 `source_merger.py`、MetaTube)。下面只列**之前未覆盖的新点**。

## 二、可借鉴提升点(新)

| # | 提升点 | OpenAver 做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **片商名规范化映射** | `maker_mapping.json`(**405 条**):日英多写法→规范名(`エスワン ナンバーワンスタイル`/`S1 NO.1 STYLE`→`S1`;`IDEA POCKET`→`IdeaPocket`),hybrid 格式 | `studio` 仅文本字段,无规范化 | **做片商维度的必备前置**:同一片商的多写法/多语言必须合并,否则会裂成多个;可借鉴这张映射表 | 🔴 高 |
| 2 | **跨语言标签别名 / 规范化** | `tag_alias` + `core/similar/canonicalize.py`:中日英同义词展开(女僕=Maid=メイド),搜索与 chip 统一 | tags 独立表,无别名/跨语言 | 标签/分类规范化 + 跨语言搜索匹配 | 🟡 中 |
| 3 | **规则式相似探索** | `core/similar/`(ranker + cache):按标签/演员/片商重叠打分推荐相似片,**本地毫秒级、免 AI 模型** | 无相似推荐 | 探索/详情页「相似片子」推荐(轻量,免模型下载) | 🟡 中 |
| 4 | 版本/规格辨识 | 文件名识别并保留 UC/LEAK/4K、VR 标记(`_180_LR`、`mkx200`) | 待对照 | 比有码无码更细的版本维度(无码破解/流出/4K/VR) | 🟢 低 |
| 5 | AI-Ready REST API | `routers/capabilities.py` 自描述 manifest,供 Claude Code/Cursor 等 agent 操作 | Tauri IPC(非 REST) | 架构不符,理念参考(自描述能力清单) | 🟢 低 |

> 旁证:`source_merger.py` 是**字段级跨源合并**的第三次印证(mediamatrix/PornBoss/OpenAver),进一步确认该范式应纳入我们的结果合并层。

## 三、关键结论

1. **最有价值的新点:规范化映射(片商名 + 标签跨语言)。** 这是做片商/系列/标签模块**绕不开的前置工程**——刮削来的片商名/标签五花八门(日英大小写、多语言),不规范化就会同物裂成多个。OpenAver 给了现成方案(405 条 maker 映射 + `canonicalize`),建议纳入「多维度浏览框架」的地基。
2. **规则式相似探索**(标签/演员/片商重叠)是轻量、免模型的「相似推荐」,直接服务探索模块的「发现喜欢的片子」。
3. 其余(字段合并、演员头像、MetaTube、有码无码)均已在既有方案中,OpenAver 是又一次印证。

## 参考来源

- [OpenAver 仓库](https://github.com/slive777/OpenAver)
- 关联:`plans/多维度浏览-统一框架方案.md`(片商/标签维度)、`plans/探索模块-全网发现方案.md`(相似探索)
