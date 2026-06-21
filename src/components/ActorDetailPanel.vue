<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { toast } from 'vue-sonner'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Loader2, Download, Star, Pencil, X, Check } from 'lucide-vue-next'
import { Input } from '@/components/ui/input'
import type { Video } from '@/types'
import { dmmCoverUrl, dmmMonoCoverUrl, isDmmPlaceholderSize, isDmmImageUrl } from '@/utils/dmm'
import { useSettingsStore, useFavoritesStore } from '@/stores'

const settingsStore = useSettingsStore()
const favoritesStore = useFavoritesStore()

// 收藏（按演员名）
const isFav = computed(() => favoritesStore.isFavorite('actor', props.actorName))
const toggleFav = () => favoritesStore.toggle('actor', props.actorName)
// 生日：截到日期、过滤零值（0001 等）。MetaTube 未知生日会回零值，需整规
const birthdayText = computed(() => {
    const raw = profile.value?.birthday
    const m = raw?.match(/(\d{4})-(\d{2})-(\d{2})/)
    if (!m || parseInt(m[1], 10) < 1900) return ''
    return `${m[1]}-${m[2]}-${m[3]}`
})

interface AliasRow {
    name: string
    lang: string
    isCanonical: boolean
}
interface Props {
    actorId: number | null
    actorName: string
    localVideos: Video[]
    // 该演员跨语言别名（中文/英文/日文/曾用名），由父组件经 entity_alias_expand 取得
    aliases?: AliasRow[]
}
const props = defineProps<Props>()
const emit = defineEmits<{
    (e: 'open-video', videoId: string): void
    (e: 'open-missing', payload: { code: string; title: string; cover: string; hasData: boolean }): void
    (e: 'refreshed'): void
    (e: 'aliases-changed'): void
}>()

// 多名字：展示（只读，不跳转）+ 编辑（加=归并、删=拉黑，均经 entity_alias 重建）
const aliasEditing = ref(false)
const newAlias = ref('')
const aliasBusy = ref(false)
const allAliases = computed<AliasRow[]>(() => props.aliases ?? [])
// 除当前查看名外的其它名字（展示态只列其它名，当前名已在标题）
const otherAliases = computed(() => allAliases.value.filter((a) => a.name !== props.actorName))
const addAlias = async () => {
    const name = newAlias.value.trim()
    if (!name || aliasBusy.value) return
    aliasBusy.value = true
    try {
        await invoke('entity_alias_force_merge', {
            entityType: 'actor',
            names: [props.actorName, name],
        })
        newAlias.value = ''
        emit('aliases-changed')
        toast.success('已添加名字')
    } catch (e) {
        toast.error('添加失败: ' + String(e))
    } finally {
        aliasBusy.value = false
    }
}
const removeAlias = async (name: string) => {
    if (aliasBusy.value || name === props.actorName) return
    aliasBusy.value = true
    try {
        await invoke('entity_alias_block', { entityType: 'actor', name })
        emit('aliases-changed')
        toast.success('已移除名字')
    } catch (e) {
        toast.error('移除失败: ' + String(e))
    } finally {
        aliasBusy.value = false
    }
}

interface ActorProfile {
    avatarPath?: string | null
    avatarUrl?: string | null
    birthday?: string | null
    height?: number | null
    cup?: string | null
    bust?: number | null
    waist?: number | null
    hip?: number | null
    workCount?: number | null
}
interface ActorWork {
    code: string
    title?: string | null
    coverUrl?: string | null
    releaseDate?: string | null
    status: string
    localVideoId?: string | null
    isUncensored: boolean
}

const profile = ref<ActorProfile | null>(null)
const works = ref<ActorWork[]>([])
const loading = ref(false)
const fetching = ref(false)
const activeTab = ref<'all' | 'local' | 'missing'>('all')

// silent=true：增量刷新时不切 loading（避免抓取过程中网格闪烁）
const loadDetail = async (silent = false) => {
    if (!props.actorId) {
        profile.value = null
        works.value = []
        return
    }
    if (!silent) loading.value = true
    try {
        const res = await invoke<{ profile: ActorProfile; works: ActorWork[] }>('get_actor_detail', {
            actorId: props.actorId,
        })
        profile.value = res.profile
        works.value = res.works ?? []
    } catch (e) {
        console.error('获取演员详情失败:', e)
    } finally {
        if (!silent) loading.value = false
    }
}

watch(
    () => props.actorId,
    () => {
        activeTab.value = 'all'
        loadDetail()
        favoritesStore.load('actor')
    },
    { immediate: true },
)

// 供父组件在缺失作品刮削落库后静默刷新网格（封面/标题即时更新）
defineExpose({ reload: () => loadDetail(true) })

const fetchProfile = async () => {
    if (!props.actorId || fetching.value) return
    fetching.value = true
    let unlisten: (() => void) | null = null
    try {
        // 边抓边显示：后端每页发进度，这里增量刷新
        unlisten = await listen<{ actorId: number; worksTotal: number }>(
            'actor-fetch-progress',
            (e) => {
                if (e.payload?.actorId === props.actorId) loadDetail(true)
            },
        )
        const r = await invoke<{ profileUpdated: boolean; worksTotal: number; worksLocal: number }>(
            'fetch_actor_profile',
            { actorId: props.actorId },
        )
        toast.success(`已抓取：${r.worksTotal} 部作品，本地 ${r.worksLocal} 部`)
        await loadDetail()
        emit('refreshed')
    } catch (e) {
        console.error('抓取演员档案失败:', e)
        toast.error('抓取失败: ' + String(e))
    } finally {
        if (unlisten) unlisten()
        fetching.value = false
    }
}

const hasWorks = computed(() => works.value.length > 0)
const localCount = computed(() => works.value.filter((w) => w.status === 'local').length)
const missingCount = computed(() => works.value.filter((w) => w.status !== 'local').length)
// 是否已抓取过（已落库）：有作品，或档案已有资料 → 按钮显示「重新抓取」
const hasFetched = computed(
    () =>
        hasWorks.value ||
        !!(profile.value && (profile.value.birthday || profile.value.height || profile.value.cup)),
)

const avatarSrc = computed<string | null>(() => {
    const p = profile.value
    if (p?.avatarPath) return convertFileSrc(p.avatarPath)
    if (p?.avatarUrl) return p.avatarUrl
    return null
})

const measurements = computed(() => {
    const p = profile.value
    if (!p) return null
    if (p.bust && p.waist && p.hip) return `${p.bust} / ${p.waist} / ${p.hip}`
    return null
})

const coverOf = (v: Video): string | null => {
    const path = v.fanart || v.poster || v.thumb
    return path ? convertFileSrc(path) : null
}

interface Card {
    key: string
    coverSrc: string | null
    code: string
    title: string
    status: 'local' | 'missing'
    videoId: string | null
    // 是否已有落库封面（区别于 DMM 兜底猜测）：有则点开缺失卡不再自动刮削
    hasStoredCover: boolean
}

// 已抓全集 → 显示全集（可切 Tab）；未抓 → 显示本地作品（来自媒体库）
const displayCards = computed<Card[]>(() => {
    if (hasWorks.value) {
        let ws = works.value
        if (activeTab.value === 'local') ws = ws.filter((w) => w.status === 'local')
        else if (activeTab.value === 'missing') ws = ws.filter((w) => w.status !== 'local')
        return ws.map((w) => ({
            key: w.code,
            // 无封面 → 用番号直拼 DMM 官方封面兜底（覆盖有码主流）
            coverSrc: w.coverUrl || dmmCoverUrl(w.code),
            code: w.code,
            title: w.title || '',
            status: w.status === 'local' ? 'local' : 'missing',
            videoId: w.localVideoId || null,
            hasStoredCover: !!w.coverUrl,
        }))
    }
    return props.localVideos.map((v) => ({
        key: v.id,
        coverSrc: coverOf(v) || dmmCoverUrl(v.localId),
        code: v.localId || '',
        title: v.title || '',
        status: 'local' as const,
        videoId: v.id,
        hasStoredCover: !!coverOf(v),
    }))
})

const onCardClick = (c: Card) => {
    if (c.videoId) emit('open-video', c.videoId)
    // 已有封面 → 直接展示不刮削；无封面 → 开即自动刮削补全。
    // 带上卡片当前封面（含 DMM 兜底）供详情展示；DMM 占位图由详情页按尺寸识别后清空再刮削补
    else if (c.code)
        emit('open-missing', {
            code: c.code,
            title: c.title,
            cover: c.coverSrc ?? '',
            hasData: c.hasStoredCover,
        })
}

// 作品卡片大小（网格 min 列宽 px）：持久化到设置（disk，重启保留）。
// 拖动过程用本地 ref 平滑更新网格，松手(@change)才写一次设置，避免频繁写配置。
const cardSize = ref(settingsStore.settings.general.actorCardSize || 160)
watch(
    () => settingsStore.settings.general.actorCardSize,
    (v) => {
        if (v && v !== cardSize.value) cardSize.value = v
    },
)
const persistCardSize = () => {
    if (cardSize.value !== settingsStore.settings.general.actorCardSize) {
        settingsStore.updateSettings({
            general: { ...settingsStore.settings.general, actorCardSize: cardSize.value },
        })
    }
}
const hideBrokenImg = (e: Event) => {
    ;(e.target as HTMLImageElement).style.visibility = 'hidden'
}

// 封面加载成功但其实是 DMM 占位图（now_printing / noimage，封面不存在时 302 跳过去的）：
// 按固定尺寸精准识别，当成加载失败处理，走 digital→mono→隐藏 兜底，不把占位图当有效封面。
const onCoverLoad = (e: Event, code: string) => {
    const img = e.target as HTMLImageElement
    const src = img.currentSrc || img.src || ''
    if (isDmmImageUrl(src) && isDmmPlaceholderSize(img.naturalWidth, img.naturalHeight)) onCoverError(e, code)
}

// 作品封面加载失败 → 依次尝试 DMM digital → mono → 隐藏。
// 能正常加载的(已有封面)不会触发，等于「已有的跳过」。WebView 自带 HTTP 缓存。
const onCoverError = (e: Event, code: string) => {
    const img = e.target as HTMLImageElement
    const cur = img.getAttribute('src') || ''
    const digital = dmmCoverUrl(code)
    const mono = dmmMonoCoverUrl(code)
    if (digital && cur !== digital && img.dataset.dmm !== 'digital' && img.dataset.dmm !== 'mono') {
        img.dataset.dmm = 'digital'
        img.src = digital
    } else if (mono && cur !== mono && img.dataset.dmm !== 'mono') {
        img.dataset.dmm = 'mono'
        img.src = mono
    } else {
        img.style.visibility = 'hidden'
    }
}
</script>

<template>
    <div class="flex h-full flex-col">
        <!-- 档案卡 -->
        <div class="flex gap-4 border-b p-4">
            <div class="size-24 shrink-0 overflow-hidden rounded-lg bg-muted">
                <img
                    v-if="avatarSrc"
                    :src="avatarSrc"
                    referrerpolicy="no-referrer"
                    class="size-full object-cover"
                    @error="hideBrokenImg"
                />
            </div>
            <div class="min-w-0 flex-1">
                <div class="flex items-center gap-2">
                    <span class="text-lg font-semibold">{{ actorName }}</span>
                    <button
                        type="button"
                        class="shrink-0 text-muted-foreground transition hover:text-yellow-500"
                        :class="isFav ? 'text-yellow-500' : ''"
                        title="收藏演员"
                        @click="toggleFav"
                    >
                        <Star class="size-5" :fill="isFav ? 'currentColor' : 'none'" />
                    </button>
                    <button
                        type="button"
                        class="shrink-0 text-muted-foreground transition hover:text-primary"
                        :class="aliasEditing ? 'text-primary' : ''"
                        title="编辑名字"
                        @click="aliasEditing = !aliasEditing"
                    >
                        <component :is="aliasEditing ? Check : Pencil" class="size-4" />
                    </button>
                </div>

                <!-- 多名字：展示态（只读，不跳转） -->
                <div v-if="!aliasEditing && otherAliases.length" class="mt-1 flex flex-wrap items-center gap-1">
                    <span
                        v-for="a in otherAliases"
                        :key="a.name"
                        class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground"
                    >{{ a.name }}</span>
                </div>
                <!-- 多名字：编辑态（加=归并、删=拉黑；属于任一名字的视频都归到本演员） -->
                <div v-if="aliasEditing" class="mt-1 flex flex-wrap items-center gap-1">
                    <span
                        v-for="a in allAliases"
                        :key="a.name"
                        class="flex items-center gap-1 rounded bg-muted px-1.5 py-0.5 text-xs"
                        :class="a.name === actorName ? 'text-foreground' : 'text-muted-foreground'"
                    >
                        {{ a.name }}
                        <button
                            v-if="a.name !== actorName"
                            type="button"
                            class="text-muted-foreground hover:text-destructive"
                            :disabled="aliasBusy"
                            title="移除"
                            @click="removeAlias(a.name)"
                        >
                            <X class="size-3" />
                        </button>
                    </span>
                    <Input
                        v-model="newAlias"
                        class="h-6 w-28 text-xs"
                        placeholder="添加名字"
                        :disabled="aliasBusy"
                        @keyup.enter="addAlias"
                    />
                </div>

                <div class="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
                    <span v-if="birthdayText">生日 {{ birthdayText }}</span>
                    <span v-if="profile?.height">身高 {{ profile.height }}cm</span>
                    <span v-if="profile?.cup">罩杯 {{ profile.cup }}</span>
                    <span v-if="measurements">三围 {{ measurements }}</span>
                </div>
                <div class="mt-2 text-sm text-muted-foreground">
                    <template v-if="hasWorks">
                        全集 {{ works.length }} 部 · 本地 {{ localCount }} · 缺失 {{ missingCount }}
                    </template>
                    <template v-else> 本地 {{ localVideos.length }} 部（未抓取全集） </template>
                </div>
                <Button size="sm" class="mt-2 gap-1" :disabled="fetching || !actorId" @click="fetchProfile">
                    <Loader2 v-if="fetching" class="size-4 animate-spin" />
                    <Download v-else class="size-4" />
                    {{ fetching ? '抓取中…' : hasFetched ? '重新抓取' : '抓取档案 / 全集' }}
                </Button>
            </div>
        </div>

        <!-- 作品 Tab + 卡片大小拖拽条 -->
        <div
            v-if="hasWorks || localVideos.length"
            class="flex items-center gap-1 border-b px-4 py-2"
        >
            <template v-if="hasWorks">
                <Button
                    v-for="t in (['all', 'local', 'missing'] as const)"
                    :key="t"
                    :variant="activeTab === t ? 'default' : 'ghost'"
                    size="sm"
                    class="h-7 text-xs"
                    @click="activeTab = t"
                >
                    {{ t === 'all' ? `全部 ${works.length}` : t === 'local' ? `本地 ${localCount}` : `缺失 ${missingCount}` }}
                </Button>
            </template>
            <div class="ml-auto flex items-center gap-2">
                <span class="text-xs text-muted-foreground">卡片</span>
                <input
                    v-model.number="cardSize"
                    type="range"
                    min="110"
                    max="300"
                    step="10"
                    class="w-28 cursor-pointer accent-primary"
                    title="封面大小"
                    @change="persistCardSize"
                />
            </div>
        </div>

        <!-- 作品网格 -->
        <ScrollArea class="min-h-0 flex-1">
            <div
                v-if="loading"
                class="flex items-center justify-center py-12 text-muted-foreground"
            >
                <Loader2 class="size-6 animate-spin" />
            </div>
            <div
                v-else-if="displayCards.length === 0"
                class="flex items-center justify-center py-12 text-sm text-muted-foreground"
            >
                暂无作品，点击「抓取档案 / 全集」获取
            </div>
            <div
                v-else
                class="grid gap-3 p-4"
                :style="{ gridTemplateColumns: `repeat(auto-fill, minmax(${cardSize}px, 1fr))` }"
            >
                <div
                    v-for="c in displayCards"
                    :key="c.key"
                    class="group"
                    :class="c.videoId || c.code ? 'cursor-pointer' : ''"
                    @click="onCardClick(c)"
                >
                    <div class="relative aspect-[3/2] overflow-hidden rounded-md bg-muted">
                        <img
                            v-if="c.coverSrc"
                            :src="c.coverSrc"
                            referrerpolicy="no-referrer"
                            loading="lazy"
                            class="size-full object-cover transition group-hover:scale-105"
                            @load="onCoverLoad($event, c.code)"
                            @error="onCoverError($event, c.code)"
                        />
                        <span
                            class="absolute right-1 top-1 rounded px-1 text-[10px] text-white"
                            :class="c.status === 'local' ? 'bg-green-600/80' : 'bg-black/60'"
                        >{{ c.status === 'local' ? '本地' : '缺失' }}</span>
                    </div>
                    <div class="mt-1 truncate text-xs font-medium" :title="c.code">{{ c.code }}</div>
                    <div class="truncate text-xs text-muted-foreground" :title="c.title">{{ c.title }}</div>
                </div>
            </div>
        </ScrollArea>
    </div>
</template>
