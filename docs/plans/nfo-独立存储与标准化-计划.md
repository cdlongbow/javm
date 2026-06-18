# 功能计划:NFO 独立目录存储 + NFO 标准化

> 状态:**已落地(编译 + 测试通过,2026-06-18)**
> 创建日期:2026-06-17
> 含两个需求:① NFO+图片存到独立目录;② NFO 对齐标准 JAV NFO。
>
> 落地范围:需求二的 `<fanart>` + `<website>` 已补;需求一(可切换独立目录 + .strm + 设置项 + 数据库封面路径)已贯通。演员头像 `<actor><thumb>` 仍待演员头像源就绪后补。

---

## 图集标准化(彻底对齐主流媒体库,2026-06-18 落地)

> 起因:旧实现「只有一个 nfo、一个封面(常为横版却命名 -poster)、几张截图」,与主流不一致。本次彻底重构。

### 主流标准(JavSP / Emby / Kodi / Jellyfin 实测)
媒体库**按文件名约定发现图集**,本地文件优先于 NFO 内 URL;JavSP 甚至不在 NFO 写图集 URL,仅靠文件名。标准文件集:

| 文件 | 方向 | 用途 |
|---|---|---|
| `<番号>-poster.jpg` | 竖 2:3 | 海报墙主图 |
| `<番号>-fanart.jpg` | 横 | 详情页背景大图 |
| `<番号>-thumb.jpg` | 横 | 横版缩略 |
| `extrafanart/fanartN.jpg` | — | 预览截图 |

本应用扫描器(`scanner/service.rs`)早已按此文件名约定读取 poster/thumb/fanart,但**刮削管线只产出一个 -poster**,故缺失 fanart/thumb 且方向语义混乱——本次补齐。

### 落地实现(清洁重构,非补丁)
- **图集权威模块** `media/artwork.rs`:`produce_artwork()` 下载横版大图作 `fanart`(+复制 `thumb`),竖版海报「源竖版优先 → 横版**右侧裁切**(`crop_imm`,378:538 比例)兜底」,保证 Emby 海报墙有真实竖版文件;`produce_artwork_from_local_image()` 供 ffmpeg 截帧走同一套(横版帧→fanart/thumb+裁竖版)。
- **图片原语** `download::image::save_image_url_to(url, path)`:统一 data/http/本地三类来源,落点由调用方决定(取代旧 `download_cover*`)。
- **NFO 图集块**(`generator.rs` `NfoArtwork`):仅引用同目录**本地相对文件名** `<poster>`/`<thumb aspect="poster">`/`<fanart><thumb></fanart>`/`<thumb aspect="landscape">`;**删除**远程封面 URL(`<cover>`/远程 `<poster>`)与预览 `<thumb>` URL(预览改走 `extrafanart/`,对齐 JavSP/MDC)。
- **三处刮削流 + 截帧(下载兜底/手动/扫描自动)**全部产出标准图集;数据库 `poster/thumb/fanart` 三列均写入,封面尺寸取横版(默认展示)代表图。
- **前端按封面方向选图**(`utils/image.ts` `resolveCoverImage`):横屏 `fanart→thumb→poster`、竖屏 `poster→fanart→thumb`,缺失互相回退不留空白;默认横版,跟随设置 `coverType`。改 VideoCard/VideoListItem/VirtualGrid/VideoDetailDialog。
- **范围**:仅新刮削/重新刮削生效,不批量迁移存量。

---

## 需求二:NFO 对齐标准 JAV NFO(改动小,先做)

当前 [generator.rs](../src-tauri/src/nfo/generator.rs) 生成的 NFO 已高度规范(`num`/`uniqueid`/`mpaa`/`set`/`maker`/`label`/`publisher`/`criticrating`/`countrycode` 齐全)。对照 JavSP / MDC / OpenAver 的标准 `movie.nfo`,**仅差 3 个字段**:

| 字段 | 标准 | 现状 | 改动 | 备注 |
|---|---|---|---|---|
| `<fanart>` | 独立横版背景图标签 | ❌ 仅 poster/cover/thumb | 小,立即可做 | 主要差异 |
| `<actor><thumb>` | 演员头像 URL | ❌ 仅 name+type | 小 | **依赖演员头像数据(暂无,后补)** |
| `<website>`/`<homepage>` | 详情页链接 | ❌ 无;`ScrapeMetadata` 未存 | 小 | 需在 `ScrapeMetadata` 加字段 |
| `uniqueid type` | 惯例 `type="num"` | `type="local"` | 极小 | 可选 |

**结论**:非重构,是「补字段」。集中在 `generator.rs`;`website` 需给 `ScrapeMetadata` 加字段;actor 头像待演员头像源就绪后补。

**✅ 落地**:`<fanart>`(Kodi 嵌套 `<thumb>` 引用横版 cover)与 `<website>`(详情页链接,`ScrapeMetadata.website` ← `SearchResult.page_url`)已补;`generator` 新增 `save_to(nfo_path)` 供独立目录直接落点。actor 头像 `<thumb>` 仍待演员头像源就绪后补;`uniqueid type="local"` 暂保持不变。

---

## 需求一:NFO + 图片存到独立目录

### 已确认的方向(经决策)
- **定位**:仍要兼容外部媒体库(Emby/Kodi/Jellyfin)。
- **目录结构**:每个番号一个子目录。

### ⚠️ 硬约束:必须配合 `.strm`
媒体库扫描目录时需看到「可播放项」才识别为影片。独立目录里没有视频,因此每个番号子目录需放一个 `.strm`(单行文本=视频真实路径),媒体库才会把它当影片并读取同目录 NFO/图片。

### 目标结构
```
<元数据根目录>/
  └─ ABC-123/
       ├─ ABC-123.strm        # 单行:视频真实绝对路径
       ├─ ABC-123.nfo
       ├─ ABC-123-poster.jpg  # NFO 内用相对文件名引用
       ├─ ABC-123-fanart.jpg
       ├─ ABC-123-thumb.jpg
       └─ extrafanart/        # 预览图
```
视频本体留原处不动;用户把媒体库指向 `<元数据根目录>`。

### 当前硬编码点(需改造)
- NFO 路径:`video_path.with_extension("nfo")`([generator.rs:246](../src-tauri/src/nfo/generator.rs))
- 图片:`{stem}-poster.jpg` 存视频父目录([assets.rs:49](../src-tauri/src/media/assets.rs))
- 预览图:视频同目录 `extrafanart/`(`EXTRAFANART_DIR_NAME`)

### 改动清单
1. **设置项**(`settings/`):新增「元数据存储模式」(跟随视频 / 独立目录)+「元数据根目录」路径。默认保持现状(跟随视频),不影响存量用户。
2. **NFO 保存**:`NfoGenerator::save()` 与 `save_nfo_for_video()` 增加「目标目录」参数;独立模式下目标=`<root>/<番号>/`,文件名 `<番号>.nfo`。
3. **图片下载/落地**:poster/thumb/fanart 与 extrafanart 的写入目录改为目标子目录;NFO 内继续用**相对文件名**引用(保证媒体库同目录可寻)。
4. **`.strm` 生成**:独立模式下,在子目录写 `<番号>.strm`,内容为视频真实绝对路径。
5. **数据库**:`videos` 表的 `dir_path`/`poster`/`thumb`/`fanart` 记录新位置(App 内展示仍能取到图)。
6. **复用**:已有 relocate/move 逻辑(assets.rs `build_artwork_target_path`/`move_optional_asset`)可部分复用。

### 改动量评估
中等。核心在 `generator.rs` + `assets.rs` + `settings/` + 数据库写入;新增 `.strm` 生成(很小)。无需重构现有同目录模式,作为可切换的第二模式叠加。

### ✅ 落地实现
- **设置项**:`AppSettings.metadata`(`MetadataSettings`:`storageMode` `follow_video`/`independent` + `rootDir`)。前端「刮削」页新增「元数据存储」卡片(模式选择 + 根目录选择器),默认跟随视频不影响存量用户。
- **目标解析层**(`media/assets.rs`):`MetadataStorageConfig` / `MediaAssetTarget` / `resolve_asset_target()` 统一决定 NFO/图片/extrafanart 落地目录与 stem;独立模式 = `<root>/<番号 标题>/`,stem = 番号;条件不满足(根目录空 / 番号空 / 路径非法)自动回退跟随视频。
- **写入定向**:新增 `download_cover_to(dir, stem)`、`sync_extrafanart_to_dir(dir)`、`save_nfo_to(dir, stem)`、`NfoGenerator::save_to(nfo_path)`;旧函数改为薄包装,跟随视频模式行为与改造前逐字节一致。
- **`.strm`**:`ensure_asset_dir_and_strm()` 在独立模式于子目录写 `<番号>.strm`(单行=视频真实绝对路径,内容不同才覆盖,幂等)。
- **三处刮削流贯通**:`rs_scrape_save` / 队列 `process_task` / 自动刮削 `perform_scrape` 均先读设置解析目标再定向落地。
- **数据库**:封面随 `local_cover_path` 写入 `videos.poster`(独立目录绝对路径,App 内仍可显示);`dir_path` 保持指向视频真实目录不变。
- **测试**:`generator` 新增 fanart/website 断言;`assets` 新增 `resolve_asset_target` / 文件名清洗等 5 项单测,全部通过。

---

## 建议实施顺序
1. **先做需求二的 `<fanart>`**(最小、立即见效,独立于需求一)。
2. **需求一**:设置项 → 目标目录贯通 NFO/图片/extrafanart → `.strm` 生成 → 数据库路径。
3. 需求二的 `website` 字段(顺带 `ScrapeMetadata` 扩展)。
4. actor 头像:等演员头像数据源就绪后补(见竞品分析的演员头像提升点)。

## 已确认(2026-06-18 决策 + 落地)
- [x] 番号子目录命名:`番号 标题/`(标题为空退化为纯番号;非法字符替换为 `_`、折叠空白、超长截断 100 字符)。
- [x] NFO 文件名:`<番号>.nfo`(与同目录 `<番号>.strm` 同名,媒体库关联最稳)。
- [x] `.strm` 仅在「独立目录模式」生成(跟随视频模式不写);内容为视频真实绝对路径,内容不同才覆盖。

## 已处理(代码审查修正,2026-06-18)
- **扫描不再覆盖独立目录封面**:扫描器原按「视频同级文件」判定封面,独立目录模式下看不到 → 会每次扫描误判无封面、重复截帧并清空库内封面。已让 `ExistingVideoScanInfo` 携带库内 poster/thumb/fanart + scan_status,扫描优先沿用「库内已记录且仍存在」的封面、已刮削项不回退状态,仅真正无封面才截帧。
- **刮削失败不再清空 thumb/fanart**:`PreparedScrapeVideo` 补全 thumb/fanart,失败回退时三件套都保留。
- **列表接口按方向给图**:`get_videos` 不再把 poster/thumb 折叠进单一 poster,改为对 poster/thumb/fanart 各自做存在性过滤后透传,配合前端按 `coverType` 选图。
- **透明图裁切**:竖版裁切统一转 RGB8 再存 JPEG,避免透明 PNG/WebP 源编码失败。
- **移动/重命名视频不再破坏独立目录、`.strm` 同步更新**:
  - 共享重定位逻辑 `resolve_asset_source` 增加「同级」过滤——只搬动与视频同级的图,独立目录里的 NFO/图**留在原地不被搬走**;`rename_video_assets_with_title` 未搬动时保留原路径,避免写库清空。`move_video_file` 的搬图同样加同级判定。
  - 新增 `sync_independent_strm()`:移动(`move_video_file`)与编辑重命名(`update_video`)后,按番号在元数据根目录下定位对应 `<番号>.strm` 并把内容改写为新视频绝对路径,外部媒体库点播不再失效。
- **手动编辑的 NFO 回写独立目录**:抽出 `find_independent_dir()`(按番号定位现有独立子目录),新增 `save_nfo_to_independent_dir()`;`update_video` 在独立目录模式下把更新后的 NFO 写回独立目录(引用该目录内已有图集),非独立 / 未找到时才回退写视频同级。手动编辑的元数据现在能被外部媒体库读到。

## 后续(未做)
- 演员头像 `<actor><thumb>`:等演员头像数据源就绪后补。
- `uniqueid type`:当前保持 `type="local"`,如需对齐惯例可改 `type="num"`(极小改动)。
- 源仅提供竖版图(无横版)时会被存为 fanart(JAV 源几乎都给横版,极罕见)。
