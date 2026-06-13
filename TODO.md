3. 设计官网，使用说明 *

4. ~~检查代码，排除冗余代码和文件 **~~

5. ~~添加 vpn 的广告位，在设置的代理界面~~

6. AI 通过视频截图，查找番号

7. 浏览器脚本和插件，***

8. 更多刮削网站

9. 更多视频链接获取网站

11. 启动密码和程序伪装

14. 招广告

16. 创建 discord 群

18. ~~刮削还是有 cf 验证问题，目前好像不正常，导致无法获取视频地址~~

19. ~~刮削时，如果有多个网站需要 cf 验证，但只有一个 webview 窗口。我希望对于同一个网站，只使用一个webview，不同的网站创建不同的webview，对此还需要设置每次最多同时存在的 刮削窗口。默认不超过3个。~~

20. ~~搜索时，没有停止功能。要求彻底停止~~

21. ~~搜索时，当前是多线程吗？可以设置，而不是默认3个~~

22. ~~当前遇到 cf webview 不显示。~~

23. ~~增强 cf 判断，只有真正的 cf 才显示 webview 。降低当前极高的误判率~~

24. ~~https://123av.com/zh/v/fsdss-498 没有资源，还是硬刮削。导致拿到的数据都是异常数据。~~

25. ~~cloudflare 验证失败，本次刮削未完成。 这个通知有问题。~~

26. ~~超时时间有点长。~~

27. ~~如果某个网站连续3次都无法通过 cf 验证，则不再启用这个网站。或者设置界面，将源网站列出来后面有开关。~~

28. ~~刮削后的数据，根据丰富度进行排序，在设置界面进行排序。不靠单词刮削结果，而是多次累加。~~

29. 尝试最新版 video.js 等待稳定

30. 反检测 增强

31. ~~增加设置项： 鼠标点击影片封面默认为打开播放~~

32. ~~增加一个筛选功能:只显示指定的目录~~

## 刮削源 HTTP 连通性测试（仅 HTTP / 无 WebView，2026-06-12）

判定标准：用 Chrome 指纹的 HTTP 客户端直接请求，能解析出结构化番号元数据为 ☑️；
仅能取到稀薄数据（标题/封面）、需 WebView、被拦截或不可达为 ❌。

已接入且 HTTP 可用：
☑️ https://javxx.to/cn        — javxx（已接入，实测 8/10 字段）
☑️ https://jav.sb/            — javsb（已接入）
☑️ https://jav.guru/         — javguru（已接入，实测 9/10 字段）
☑️ https://javtiful.com/main — javtiful（已接入）
☑️ https://123av.com/zh/dm9  — 123av（已接入，实测 8/10 字段）
☑️ https://cn.myjav.tv/      — myjav（已接入）
☑️ https://javgg.net/        — 新增 javgg 数据源；HTTP 直接可解析（标题/演员/片商/日期/时长/类别，实测 7/10）
☑️ https://www.javmost.ws/   — 新增 javmost 数据源；HTTP 直接可解析（标题/演员/片商/日期/类别，实测 6/10）

未接入（HTTP 下不适合作为元数据源）：
❌ https://thisav2.com/dm194/cn      — 流媒体站，需 WebView 取播放链接，元数据稀薄
❌ https://cn.javd.me/               — HTTP 取不到结构化番号数据
❌ https://xchina.co/                — 图集站，非番号元数据
❌ https://jav.rip/                  — 连接失败 / 不可达
❌ https://javquick.com/             — 流媒体站，元数据稀薄
❌ https://www.njav.com/zh/          — 流媒体站，元数据稀薄
❌ https://javct.net/                — 流媒体站，元数据稀薄
❌ https://javcl.com/                — HTTP 404
❌ https://javmod.com/               — HTTP 取不到番号数据
❌ https://javpain.com/              — HTTP 取不到番号数据
❌ https://javeng.tv/                — 流媒体站，元数据稀薄
❌ https://www.bestjavporn.com/zh/   — 301 跳转，未取到数据
❌ https://jav.wine/                 — 流媒体站，元数据稀薄
❌ https://jav.spa/                  — 流媒体站，元数据稀薄
❌ https://jpsub.net/                — 字幕 / 流媒体站，元数据稀薄
❌ https://javhoc.com/               — HTTP 取不到番号数据

注：上面多数 ❌ 属"视频链接获取站"（TODO 9），其核心是播放链接，需 WebView 抓取，
不在本次"仅 HTTP 元数据刮削"范围内。

## 追加候选站点（去重后，2026-06-12）

已接入 / 已测（见上方表，不再重复）：
missav* · javtiful · javlibrary · javbus · javmenu · 123av · jav.guru · jav.sb · javgg ·
myjav · javxx · javmost(.ws/.com/.cx) · njav · bestjavporn · javcl · javct

元数据库型测试结果（HTTP-only，番号 ssis-666/777/888）：
☑️ sextb.net        — 新增 sextb 数据源；详情 /{CODE} 直出，实测 6/10（标题/演员/封面/片商/导演/类别）
❌ javdb.com        — 403 / Cloudflare 拦截，需 WebView
❌ jav321.com       — 仅支持 POST 搜索，GET 取不到
❌ avmoo.com        — 302 跳转，HTTP 取不到
❌ avsoox.com       — 连接失败 / 不可达
❌ airav.cc         — 301 跳转、API 路径不符，HTTP 未取到结构化数据
❌ javtrailers.com  — 搜索页可达但无 og 封面、详情 URL 未定，元数据偏弱
❌ 7mmtv.sx         — 403 拦截
❌ maxjav.com       — 下载站，元数据稀薄
❌ javcen.com       — 元数据稀薄
❌ javfinder / javseen — 域名/路径不可达

待测·其余（多为在线观看站，核心是播放链接，需 WebView → 归 TODO 9）：
supjav · javdock · javhaven · tktube · javraveclub · javpornhd · new-jav · javhd.com ·
javhd.today / javhdporn · javenglish.cc · javtsunami · vjav · hpjav · javhub · 24av ·
javsub · javhd.icu · javangel · javpub · ichiav · pussyav · javplatform · javfor.me ·
avgle · javdoe · javwhores · erito · tokyomotion · jable.tv · netflav · freejavonline ·
fyav · fujiav · javfree · javmix · bteat · javme.xyz · javhihi · javbangers · javfull ·
javbests · javplayer · javdaddy

## 视频下载链接 HTTP 连通性测试（番号 ssis-666/777/888，2026-06-13）

工具：`cargo run --example link_check`（Chrome 指纹 HTTP 客户端 + 代理，与刮削体检
`parser_check` 分离）。判定：HTTP 抓到的页面含视频信号（.m3u8 / .mp4 / source / 播放器库 /
iframe），任一番号命中即 ☑️。

**重要局限**：纯 HTTP 不执行 JS。锚点 jable、missav（应用内 WebView 实际可用）在 HTTP 下
均 403，说明视频站多有 CF/JS 保护、播放地址由脚本注入。故 ❌ 仅表示"HTTP 抓不到"，
**不代表应用内 WebView 也拿不到**；视频链接站的权威测试仍以 WebView 查找为准。

纯 HTTP 即可命中视频信号（☑️）：
☑️ thisav2 — mp4/source/iframe（已对应查找器 thisav）
☑️ njav    — mp4（已在查找器）
☑️ javct   — 播放器库（已在查找器）
☑️ tktube  — mp4（🆕 未接入）
☑️ vjav    — 播放器库（🆕）
☑️ javhub  — mp4/iframe（🆕）
☑️ erito   — mp4/播放器库（🆕）
☑️ ichiav  — iframe（🆕）
☑️ netflav — iframe（🆕）

HTTP 下 ❌（被拦/跳转/不可达/无信号，未必 WebView 不可用）：
supjav(403) · jable(403) · missav(403) · javdock(301) · javhdporn(301) · hpjav(302) ·
24av(301) · pussyav(302) · tokyomotion(301) · javbangers(301) · javeng(301) · jav.wine(301) ·
javquick(404) · jav.spa(404) · new-jav/javpub/avgle/javwhores/freejavonline/fujiav/javfree/
bteat/javbests/javdaddy(不可达) · javhaven/javraveclub/javpornhd/javhd.today/javenglish/
javtsunami/javsub/javhd.icu/javangel/javplatform/javfor.me/javdoe/javmix/javme.xyz/javhihi/
javfull/javplayer/fyav(200 但无信号，可能 URL 猜测不符或 JS 注入)

## 视频链接 隐藏 WebView 实测（番号 SSIS-666，2026-06-13）

工具：`JAVM_LINK_PROBE=link_probe_targets.json cargo run`（dev 专用，见
`src-tauri/src/resource_scrape/link_probe.rs`）。用应用真实的隐藏 WebView 流程
（反检测脚本 + INTERCEPT_JS 拦截 + 自动跳转搜索结果 + 触发播放）批量探测约 40 个在线站。

能抓到视频链接（☑️）：
☑️ missav / jable / njav — 锚点，已在查找器
☑️ javhub  — 抓到 mp4，但详情页是 token 链接 /play/{token}/，按番号搜索未精确命中，需先搜索解析
☑️ javfull — https://javfull.net/{code}/ 直达详情、抓到 mp4（番号 slug，干净，适合接入）

到达正确详情页但抓不到（跨域 iframe 播放器，顶层注入的固有限制，应用同样抓不到）：
❌ javhdporn(/zh/video/{code}/) — 页面存在但播放器在跨域 iframe 内

其余 ❌ 原因（由 final_url 诊断）：
- 无该番号结果 / 搜索页 JS 未出结果：supjav · erito · netflav · javhaven · javraveclub ·
  javhd.icu · javfor.me · javme.xyz · javhihi · javplatform · javplayer · freejavonline
- 被拦 / 跳转 / 域名变更：tktube(→login) · 24av(→域名跳转) · javhd.com(404) ·
  tokyomotion(搜索参数不符) · avgle/hpjav/javdock/javbangers(about:blank，加载失败)
- 跳到无关页：vjav(latest-updates) · ichiav/pussyav/new-jav(跟进到非目标视频)

结论：在线站点绝大多数要么不收录该番号、要么被 CF/跳转挡住、要么播放器在跨域 iframe
（顶层拦截抓不到，与应用一致）。按番号 slug 直达且能抓到的新站只有 **javfull**。
探测框架已就位，后续换番号/加候选改 link_probe_targets.json 重跑即可，无需人工点 UI。