# 对标分析:javspider_stack

- **仓库**:https://github.com/hk-raints/javspider_stack
- **分析日期**:2026-06-17
- **分析维度**:反爬工具箱、异步任务调度
- **规模**:Python(FastAPI 异步 + BeautifulSoup),~5 星,JavBus 单源
- **定位**:本地优先 JAV 元数据管理 + 异步爬虫 + WebSocket 实时任务看板

## 一、定位与先说结论

刮削本身(JavBus 单源)与已分析项目雷同,无新意。它的工程化(全异步、WebSocket 看板、Alembic 迁移)是架构方向参考。**真正可借鉴的新东西集中在「反爬工具箱」**(`core/anti_block.py` + `core/http_client.py`)。

⚠️ 注意:它的 `pipeline_manager.py` 自述「借鉴自 jav-scrapy」,且其 `ResourceMonitor`(CPU/内存过载→降并发)**本项目已有等价物且更底层**——`adaptive_concurrency.rs` 已基于 CPU 负载用 sysinfo 每 3 秒采样动态调并发。故自适应并发我们不缺。

## 二、反爬工具箱(`core/anti_block.py`、`http_client.py`)

| 组件 | 做法 |
|---|---|
| `UserAgentRotator` | UA 池,随机 / 轮询切换 |
| `ProxyRotator` | **代理池按成功率加权选取**:每个代理记 `success_rate`,`mark_success/fail` 调整,`random.choices(weights=...)` 加权随机,`min_success_rate=0.5` 过滤失效代理 |
| `URLRotator` | **多镜像 base_url 轮换 + 文件缓存**(可增删),对应 JavBus 镜像域名容错 |
| `_wait_for_delay`(http_client) | **请求间隔限速**:记 `last_request_time`,每次随机 `REQUEST_DELAY_MIN~MAX` 延迟,不足则 sleep,礼貌爬取防封 |
| 分级退避重试 | 429 读 `Retry-After` sleep;其它错误 `5*attempt` / `10*attempt` 递增退避 |
| `_verify_age` | 自动过 18+ 年龄确认页 |

## 三、可借鉴提升点

| # | 提升点 | 它的做法 | 我们现状 | 建议 | 优先级 |
|---|---|---|---|---|---|
| 1 | **成功率加权代理池** | 代理按成功率加权选取,失败降权、低于阈值过滤 | wreq + 可选单代理,无加权池 | 多代理时智能选优、自动避开失效代理 | 🟡 中 |
| 2 | **多镜像 URL 轮换 + 缓存** | `URLRotator` 多 base_url + 文件缓存 | 反爬源镜像策略待对照(JableTV 已有 fs1.app 镜像) | JavBus/MissAV 等镜像域名轮换容错,可缓存可用域名 | 🟡 中 |
| 3 | **请求限速 + 分级退避** | 随机请求间隔 + 429 读 Retry-After + 递增退避 | 待对照 fetcher 是否已有 | fetcher 加礼貌限速 + 分级退避,降低封禁/被限频 | 🟡 中 |
| 4 | 资源监控自适应并发 | `ResourceMonitor` CPU+内存过载降并发 | **已有 `adaptive_concurrency.rs`(CPU)** | 已具备;可补内存维度 + 复用到 WebView 池 | 🟢 低(已有) |
| 5 | 演员头像批量下载 | `scripts/download_actress_avatars.py` | 缺 | **第三次印证**演员头像(见 nassav.md:JavBus 详情页直接抓) | 🟢 低(已覆盖) |

## 四、关键结论

1. **自适应并发我们已领先**(Rust + sysinfo CPU 监控限制器),无需借鉴;最多补「内存维度」并复用到 WebView 池。
2. **真正值得拿的是反爬工具箱**:代理池成功率加权(#1)、镜像 URL 轮换+缓存(#2)、请求限速+分级退避(#3)——都是降低封禁、提升爬取稳定性的成熟手段,可增强现有 fetcher。
3. **演员头像第三次出现**(专门的批量下载脚本),再次确认这是高共识、应补的能力。

## 参考来源

- [javspider_stack 仓库](https://github.com/hk-raints/javspider_stack)
- 关联:`docs/competitor-analysis/nassav.md`、`src-tauri/src/utils/adaptive_concurrency.rs`
