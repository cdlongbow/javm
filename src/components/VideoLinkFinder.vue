<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Loader2,
  Download,
  Link as LinkIcon,
  X,
  Search,
  Play,
} from 'lucide-vue-next'
import {
  findVideoLinks,
  closeVideoFinder,
  closeAllVideoFinders,
  getVideoSites,
  openVideoPlayerWindow,
  analyzeHls,
  type VideoLink,
  type VideoSite,
  type FinderLinkEvent,
  type FinderPageStateEvent,
  type FinderCfStateEvent,
  checkVideoExists,
  type VideoExistCheckResult,
} from '@/lib/tauri'
import { getDefaultDownloadPath } from '@/lib/tauri'
import { useDownloadStore, useSettingsStore } from '@/stores'
import { createScheduler } from '@/composables/useParallelFinder'
import { toast } from 'vue-sonner'

// 状态
const downloadStore = useDownloadStore()
const settingsStore = useSettingsStore()
const code = ref('')
const savePath = ref('')
const adding = ref(false)
const sites = ref<VideoSite[]>([])

let unlisten: UnlistenFn | null = null
let unlistenCf: UnlistenFn | null = null
let unlistenPageState: UnlistenFn | null = null

// 下载查重状态
const duplicateCheckOpen = ref(false)
const duplicateVideoInfo = ref<VideoExistCheckResult['video']>()
// 保存当前等待确认的回调
const pendingDownloadContext = ref<{ type: 'single'; link: VideoLink; siteId: string } | { type: 'batch' } | null>(null)

// 开发者模式显示真实站点名，否则一律用代号「资源 N」，不暴露真实网站名
const isDeveloperMode = import.meta.env.DEV

// 并行调度器
const scheduler = createScheduler({
  open: (c, site) => findVideoLinks(c, site),
  close: (site) => closeVideoFinder(site),
  closeAll: () => closeAllVideoFinders(),
  analyze: (url) => analyzeHls(url),
  concurrency: settingsStore.settings.scrape.linkFinderConcurrency ?? 3,
  timeoutSecs: settingsStore.settings.scrape.linkFinderSourceTimeoutSecs ?? 120,
})
const sources = scheduler.sources
const running = scheduler.running

// 进度统计
const total = computed(() => sources.value.length)
const searchedCount = computed(() =>
  sources.value.filter((s) => ['found', 'failed', 'notfound'].includes(s.status)).length,
)
const foundCount = computed(() => sources.value.filter((s) => s.status === 'found').length)

// 能否开始查找
const canStart = computed(() => code.value.trim().length > 0 && !running.value)

function formatDuration(secs?: number): string {
  if (!secs || secs <= 0) return ''
  const s = Math.round(secs)
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const sec = s % 60
  return h > 0
    ? `${h}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`
    : `${m}:${String(sec).padStart(2, '0')}`
}

function formatRes(link: VideoLink): string {
  if (link.height && link.height > 0) return `${link.height}p`
  return link.resolution ?? ''
}

// 下载路径直接取自下载设置；未配置时回退到系统默认下载目录
async function resolveSavePath() {
  const configured = settingsStore.settings.download.savePath
  if (configured) {
    savePath.value = configured
    return
  }
  try {
    savePath.value = await getDefaultDownloadPath()
  } catch { /* 忽略 */ }
}

// 开始查找
async function startFinding() {
  const c = code.value.trim().toUpperCase()
  if (!c) return

  // 先解绑上一轮，避免监听器叠加
  if (unlisten) { unlisten(); unlisten = null }
  if (unlistenCf) { unlistenCf(); unlistenCf = null }
  if (unlistenPageState) { unlistenPageState(); unlistenPageState = null }

  try { sites.value = await getVideoSites() } catch { /* 忽略 */ }
  await resolveSavePath()

  const siteIds = sites.value.map((s) => s.id)

  try {
    unlisten = await listen<FinderLinkEvent>('video-finder-link', (e) => scheduler.onLink(e.payload))
    unlistenPageState = await listen<FinderPageStateEvent>('video-finder-page-state', (e) => scheduler.onPageState(e.payload))
    unlistenCf = await listen<FinderCfStateEvent>('video-finder-cf-state', (e) => scheduler.onCfState(e.payload))
  } catch (e) {
    console.error('监听事件失败:', e)
    toast.error('启动监听失败，无法查找链接')
    return
  }

  scheduler.start(c, siteIds)
}

// 停止查找
function stopFinding() {
  scheduler.stop()
  if (unlisten) { unlisten(); unlisten = null }
  if (unlistenCf) { unlistenCf(); unlistenCf = null }
  if (unlistenPageState) { unlistenPageState(); unlistenPageState = null }
}

// 回车键触发查找
function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && canStart.value) startFinding()
}

// 预览 HLS 视频
async function handlePreview(link: VideoLink) {
  if (!link.isHls) {
    toast.error('该链接不是 HLS，无法直接预览')
    return
  }
  const title = `${code.value.trim().toUpperCase()} - 在线预览`
  try {
    await openVideoPlayerWindow(link.url, title, true)
  } catch (e) {
    toast.error('打开预览失败: ' + String(e))
  }
}

// 下载单个视频
async function handleDownloadSingle(link: VideoLink, siteId: string, ignoreDuplicate: boolean = false) {
  if (!savePath.value) {
    toast.error('未设置默认下载路径，请在系统设置 - 下载设置中配置')
    return
  }
  const filename = code.value.trim().toUpperCase()

  if (!ignoreDuplicate) {
    try {
      const checkResult = await checkVideoExists(filename)
      if (checkResult.exists) {
        duplicateVideoInfo.value = checkResult.video
        pendingDownloadContext.value = { type: 'single', link, siteId }
        duplicateCheckOpen.value = true
        return
      }
    } catch (e) {
      toast.error(`查重失败: ${e}`)
      return
    }
  }

  try {
    await downloadStore.addTask(link.url, savePath.value, filename, siteId)
    toast.success('已添加下载任务')
  } catch {
    toast.error('添加任务失败（已存在）')
  }
}

// 确认强制下载
function forceDownload() {
  duplicateCheckOpen.value = false
  if (pendingDownloadContext.value?.type === 'batch') {
    void handleAddTasks(true)
  } else if (pendingDownloadContext.value?.type === 'single') {
    void handleDownloadSingle(pendingDownloadContext.value.link, pendingDownloadContext.value.siteId, true)
  }
  pendingDownloadContext.value = null
}

// 取消下载
function cancelDownload() {
  duplicateCheckOpen.value = false
  pendingDownloadContext.value = null
}

// 批量添加所有正片下载任务
async function handleAddTasks(ignoreDuplicate: boolean = false) {
  const foundSources = sources.value.filter((s) => s.status === 'found' && s.realLink)
  if (foundSources.length === 0 || !savePath.value) return

  const filename = code.value.trim().toUpperCase()

  if (!ignoreDuplicate) {
    try {
      const checkResult = await checkVideoExists(filename)
      if (checkResult.exists) {
        duplicateVideoInfo.value = checkResult.video
        pendingDownloadContext.value = { type: 'batch' }
        duplicateCheckOpen.value = true
        return
      }
    } catch (e) {
      toast.error(`查重失败: ${e}`)
      return
    }
  }

  adding.value = true
  let success = 0
  let failed = 0

  for (const s of foundSources) {
    if (!s.realLink) continue
    try {
      await downloadStore.addTask(s.realLink.url, savePath.value, filename, s.siteId)
      success++
    } catch { failed++ }
  }

  adding.value = false
  if (success > 0) toast.success(`已添加 ${success} 个下载任务`)
  if (failed > 0) toast.error(`${failed} 个任务添加失败（已存在）`)
}

// 复制下载链接到剪贴板
async function copyDownloadLink(url: string) {
  try {
    await navigator.clipboard.writeText(url)
    toast.success('下载链接已复制到剪贴板')
  } catch (e) {
    toast.error('复制失败: ' + String(e))
  }
}

// 暴露给父组件的方法
defineExpose({
  autoSearch: (newCode: string) => {
    code.value = newCode
    setTimeout(() => {
      if (canStart.value) startFinding()
    }, 100)
  }
})

onMounted(async () => {
  try { sites.value = await getVideoSites() } catch { /* 忽略 */ }
  await resolveSavePath()
})

onUnmounted(() => {
  stopFinding()
})
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- 输入区域 -->
    <div class="flex items-center gap-2 border-b p-4">
      <Input v-model="code" placeholder="输入番号，如 ABC-123（需要科学上网）" class="max-w-xs" :disabled="running"
        @keydown="handleKeydown" />
      <Button v-if="!running" :disabled="!canStart" size="sm" @click="startFinding">
        <Search class="mr-2 size-4" />
        查找链接
      </Button>
      <Button v-else variant="outline" size="sm" @click="stopFinding">
        <X class="mr-2 size-4" />
        停止
      </Button>

      <!-- 进度摘要 -->
      <span v-if="total > 0" class="text-xs text-muted-foreground tabular-nums">
        已搜 {{ searchedCount }}/{{ total }} · 正片 {{ foundCount }}
      </span>
    </div>

    <!-- 内容区域 -->
    <div class="flex-1 flex flex-col min-h-0 p-4 gap-3">
      <!-- 未开始 -->
      <div v-if="!running && sources.length === 0"
        class="flex flex-col items-center justify-center py-16 gap-3 text-muted-foreground">
        <LinkIcon class="size-8 opacity-30" />
        <span class="text-sm">输入番号并点击查找，自动捕获候选下载链接，需要科学上网</span>
      </div>

      <!-- 按源列表 -->
      <div v-if="sources.length > 0" class="flex-1 flex flex-col min-h-0 gap-2 overflow-y-auto">
        <div
          v-for="(s, i) in sources"
          :key="s.siteId"
          class="rounded-md border p-2.5"
        >
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium">{{ isDeveloperMode ? s.siteId : `资源 ${i + 1}` }}</span>
            <Badge v-if="s.status === 'searching'" class="gap-1">
              <Loader2 class="size-3 animate-spin" />
              搜索中
            </Badge>
            <Badge v-else-if="s.status === 'cf'" variant="secondary">CF 验证中</Badge>
            <Badge v-else-if="s.status === 'found'" class="bg-green-600 text-white">✓ 正片</Badge>
            <Badge v-else-if="s.status === 'notfound'" variant="secondary">404</Badge>
            <Badge v-else-if="s.status === 'failed'" variant="secondary">✗ 失败</Badge>
            <Badge v-else variant="outline">等待中</Badge>
          </div>
          <div v-if="s.realLink" class="mt-1.5 flex items-center gap-2">
            <Badge v-if="s.realLink.height" variant="outline" class="shrink-0">{{ s.realLink.height }}p</Badge>
            <Badge v-if="s.realLink.durationSecs" variant="outline" class="shrink-0 tabular-nums">{{ formatDuration(s.realLink.durationSecs) }}</Badge>
            <Badge v-if="formatRes(s.realLink) && !s.realLink.height" variant="outline" class="shrink-0">{{ formatRes(s.realLink) }}</Badge>
            <span class="font-mono text-xs text-muted-foreground break-all flex-1">{{ s.realLink.url }}</span>
            <Button
              v-if="s.realLink.isHls"
              size="icon"
              variant="ghost"
              class="h-8 w-8 shrink-0"
              title="预览播放"
              @click="handlePreview(s.realLink)"
            >
              <Play class="size-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              class="h-8 w-8 shrink-0"
              title="下载资源"
              @click="handleDownloadSingle(s.realLink, s.siteId)"
            >
              <Download class="size-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              class="h-8 w-8 shrink-0"
              title="复制下载链接"
              @click="copyDownloadLink(s.realLink.url)"
            >
              <LinkIcon class="size-4" />
            </Button>
          </div>
        </div>
      </div>

      <!-- 保存路径和批量下载（有正片时显示） -->
      <div v-if="foundCount > 0" class="flex items-center gap-2 mt-auto pt-2 border-t">
        <div class="flex-1 min-w-0 truncate text-xs text-muted-foreground">
          保存到：{{ savePath || '未设置默认下载路径，请在系统设置 - 下载设置中配置' }}
        </div>
        <Button :disabled="!savePath || adding" size="sm" class="h-9 shrink-0"
          @click="() => handleAddTasks(false)">
          <Loader2 v-if="adding" class="mr-2 size-4 animate-spin" />
          <Download v-else class="mr-2 size-4" />
          下载全部正片（{{ foundCount }}）
        </Button>
      </div>
    </div>

    <!-- 重复提醒弹窗 -->
    <Dialog :open="duplicateCheckOpen" @update:open="(v) => !v && cancelDownload()">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>检测到已存视频</DialogTitle>
          <DialogDescription>
            该番号的视频 <strong>{{ code.trim().toUpperCase() }}</strong> 已存在于媒体库中。
          </DialogDescription>
        </DialogHeader>
        <div v-if="duplicateVideoInfo" class="space-y-4 py-4 text-sm text-muted-foreground break-all">
          <div>标题：<span class="text-foreground">{{ duplicateVideoInfo.title || '未知' }}</span></div>
          <div>目录：<span class="text-foreground">{{ duplicateVideoInfo.videoPath }}</span></div>
        </div>
        <DialogFooter class="flex sm:justify-end gap-2 text-right">
          <Button type="button" variant="secondary" @click="cancelDownload">取消添加</Button>
          <Button type="button" variant="destructive" @click="forceDownload">忽略并强制下载</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
