# 对标分析:CodeSeek

- **仓库**:https://github.com/Zoroaaa/codeseek
- **分析日期**:2026-06-17
- **分析维度**:磁力链接获取机制
- **规模**:TypeScript,~55 星,活跃维护
- **定位**:开源聚合搜索引擎(JAV/动漫/影视),Cloudflare Workers + D1 无服务器架构

## 一、磁力获取机制 —— 纯爬取,非自建服务

CodeSeek 的 JAV 磁力**完全从 JavBus / JavDB 页面爬取解析**,不涉及自建 DHT、不调用付费磁力 API。代码(`backend/src/providers/jav-provider.ts` + `services/jav-utils.ts`)揭示的 JavBus 流程:

**关键点:JavBus 磁力不在详情页 HTML 里,而是 JS 异步从 ajax 接口加载。** 三步:

1. **抓详情页** `https://www.javbus.com/{番号}`,正则提取内联 JS 变量:
   ```js
   var gid = (\d+)   // extractGidUc
   var uc  = (\d+)
   ```
2. **请求磁力 ajax 接口**(需带 Referer = 详情页):
   ```
   https://www.javbus.com/ajax/uncledatoolsbyajax.php?lang=zh&gid={gid}&uc={uc}&floor={timestamp}
   ```
3. **解析返回 HTML 表格行** → 每条磁力:
   ```ts
   interface MagnetItem {
     name: string;   // 磁力名
     size: string;   // 大小(第2列)
     date: string;   // 日期(第3列)
     magnet: string; // magnet:?xt=... ; 正则 href="(magnet:\?xt=[^"]+)"
     isHD: boolean;  // name 含 hd/1080/720 或行内 btn-primary
   }
   ```

其余来源:JavDB(磁力在详情页内)、TPB/YTS/EZTV/Nyaa(影视/动漫,不相关)。Provider 注册模式 + 源状态监控自动跳过失效源。

## 二、我们能否补上 —— 能,成本很低

前置条件已具备:
- 已在爬 JavBus(16 源之一),已有反爬 fetcher(wreq + TLS 指纹)和 javbus 解析器。
- 补磁力是**纯增量**:现有 javbus 详情解析后多走一步 ajax,无需外部服务/DHT/第三方 API。

### 实现要点(复用现有 javbus 源)
1. javbus 详情 HTML 正则提取 `gid`/`uc`。
2. 拼 `uncledatoolsbyajax.php` 接口,**带 Referer 头**请求(关键,否则可能被拒;可能需 cookie)。
3. 解析表格行 → magnet/size/date/HD/字幕。

### 增强点(比 CodeSeek 更全)
- CodeSeek 只取了 HD 标记;JavBus 磁力行通常还有**字幕**标记(`btn-warning`),建议一并提取。
- 磁力可入库并在影片详情页展示/复制/推送到下载器(项目已有 download 模块 + N_m3u8DL-RE)。

### 风险
- 依赖 `gid`/`uc` 内联变量 + ajax 接口结构,JavBus 改版会失效(同所有 HTML 爬取)。
- JavDB 磁力需过 Cloudflare,可作第二来源。
- 合规:磁力为 P2P 分享链接,与现有刮削同性质(个人使用定位)。

## 三、可借鉴提升点

| # | 提升点 | CodeSeek 做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **磁力链接获取** | 爬 JavBus 详情页 + `uncledatoolsbyajax.php` ajax 接口解析磁力 | 无磁力功能 | 复用 javbus 源增量补磁力(详情→gid/uc→ajax→解析) | 🔴 高 |
| 2 | 磁力质量标记 | 仅 isHD | — | 同时提取 HD + 字幕标记,按质量排序 | 🟡 中 |
| 3 | 多源磁力聚合 | JavBus + JavDB | — | JavBus 为主,JavDB 兜底(需过 CF) | 🟢 低 |

## 参考来源

- [CodeSeek 仓库](https://github.com/Zoroaaa/codeseek)
