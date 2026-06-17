# 对标分析:hyperq/jav

- **仓库**:https://github.com/hyperq/jav
- **分析日期**:2026-06-17
- **分析维度**:磁力获取与排序、有码/无码切换
- **规模**:Rust,~42 星,JavBus 的终端 TUI 客户端(ratatui)
- **价值**:**与本项目后端同语言(Rust),且用 `scraper` crate(本项目已用),实现几乎可直接参考/移植**

## 一、磁力获取 + 排序

### 获取(`src/scraper/client.rs::fetch_magnets`)
与 CodeSeek 同一 JavBus 机制(再次印证),但更完整——多取一个 `img` 变量:
1. 详情页 `parse_script_vars` 提取 JS 变量 **`gid` / `uc` / `img`**(CodeSeek 仅 gid/uc)。
2. 构造 ajax:
   ```
   {base}/ajax/uncledatoolsbyajax.php?{gid}&lang=zh&{img}&{uc}&floor={rand u16 % 1000}
   ```
3. 返回 HTML 片段用 `<table>` 包裹,`Selector::parse(r#"a[href^="magnet"]"#)` 解析每行。
4. 每条:`Magnet { link, size, caption }`——`caption`=该行含「字幕」,`size` 带 HD 标记(`{size} HD`)。

### 排序(README 声明)
**字幕 > 高清 > 文件大小** 三级优先,自动选最优磁链(`g` 键复制,`e` 键导出)。`Magnet` 的 `caption`(字幕 bool)+ `size`(含 HD)正好支撑该排序键。

### 附带信息
列表带 `result_info`:`"已有磁力 194 / 全部影片 399"`——直观展示磁力覆盖率。

## 二、有码/无码切换 —— JavBus URL 前缀

`build_list_url_ex` / `build_actress_url`:就是加不加 `/uncensored` 前缀。

| 维度 | 有码 | 无码 |
|---|---|---|
| 列表 | `{base}/...` | `{base}/uncensored/...` |
| 搜索 | `/search/{kw}` | `/uncensored/search/{kw}&type=1` |
| 女优 | `parent=ce` | `parent=uc` |

JavBus 站点本身用 `/uncensored/` 分库,切换=切 URL 前缀。简单可靠。

## 三、可借鉴提升点

| # | 提升点 | 它的做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **磁力获取(Rust 可直接参考)** | JavBus `uncledatoolsbyajax`(gid/uc/img + floor 随机)+ `scraper` 解析 | 无磁力 | javbus 源用同机制补磁力——**Rust + scraper 与本项目栈一致,可直接移植** | 🔴 高 |
| 2 | **磁力排序** | 字幕 > 高清 > 文件大小,自动选最优 | — | 磁力按三级键排序,默认推荐最优 | 🟡 中 |
| 3 | **有码/无码切换** | JavBus `/uncensored` URL 前缀(搜索 `type=1`,女优 `parent=uc/ce`) | 不区分 | 列表/搜索/详情区分两套 URL(印证 PornBoss 的有码无码分轨) | 🟡 中 |
| 4 | 磁力覆盖率提示 | `已有磁力 X / 全部影片 Y` | — | 列表展示有磁力比例 | 🟢 低 |
| 5 | 推送 115 离线下载 | `src/cloud115/offline.rs` 扫码登录推送磁力 | — | 网盘离线下载(呼应 JavHub 的 115 集成) | 🟢 低 |

## 四、关键结论

1. **磁力机制第二次被印证**(CodeSeek + 本项目都用 JavBus `uncledatoolsbyajax`),且这个 Rust 版更完整(带 `img` 参数)、用的就是本项目的技术栈,**移植成本最低**。
2. **磁力排序**(字幕>高清>大小)和**有码/无码 URL 前缀切换**都是简单可靠的小设计,可直接采纳。
3. 与 `docs/plans/视频获取-快路径与并发优化方案.md` 的磁力部分、`docs/competitor-analysis/codeseek.md` 互为补充。

## 参考来源

- [hyperq/jav 仓库](https://github.com/hyperq/jav)
