import { onActivated, onBeforeUnmount, onDeactivated, onMounted } from 'vue'

// 模块级存储：跨组件挂载/卸载、跨路由(KeepAlive 停用/激活)都持久，按 key 区分不同列表
const positions = new Map<string, number>()

/**
 * 记忆并恢复滚动容器位置（发现页各视频列表用）。
 *
 * - 滚动时**持续记录**为权威值：KeepAlive 停用时 Vue 已把 DOM 移入分离容器、scrollTop 多被重置为 0，
 *   不能等到那时才读，否则会把真实进度清零。
 * - 挂载 / KeepAlive 激活 / 手动 `restore()`（如切换取值后内容重载）时，按 key 用双 rAF 恢复
 *   （等本帧布局落地后再设，避免早于布局导致设不上）。
 *
 * @param getViewport 返回实际可滚动元素（如 ScrollArea 的 viewport）
 * @param getKey 返回当前列表的 key（如 `facet:genre:足交`）
 */
export function useScrollMemory(
    getViewport: () => HTMLElement | null,
    getKey: () => string,
) {
    let raf = 0
    let bound: HTMLElement | null = null

    const onScroll = () => {
        if (raf) return
        raf = requestAnimationFrame(() => {
            raf = 0
            const el = getViewport()
            if (el) positions.set(getKey(), el.scrollTop) // 含 0：滚回顶部也如实记录
        })
    }
    const bind = () => {
        const el = getViewport()
        if (el && el !== bound) {
            bound?.removeEventListener('scroll', onScroll)
            el.addEventListener('scroll', onScroll, { passive: true })
            bound = el
        }
    }
    // 仅在能读到有效滚动值时保存（停用/卸载时 DOM 可能已分离、读到 0，不可覆盖真实进度）
    const save = () => {
        const el = getViewport()
        if (el && el.scrollTop > 0) positions.set(getKey(), el.scrollTop)
    }
    const restore = () => {
        bind()
        const y = positions.get(getKey()) ?? 0
        requestAnimationFrame(() => {
            const el = getViewport()
            if (el) el.scrollTop = y
            requestAnimationFrame(() => {
                const el2 = getViewport()
                if (el2) el2.scrollTop = y
            })
        })
    }

    onMounted(restore)
    onActivated(restore)
    onDeactivated(save)
    onBeforeUnmount(() => {
        save()
        bound?.removeEventListener('scroll', onScroll)
    })

    return { restore, save }
}
