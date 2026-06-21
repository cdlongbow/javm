import { ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'

/**
 * 收藏：演员/片商/系列/导演/分类 五个维度，按（维度类型, 取值名）收藏。
 * 维度值列表是按名聚合派生的，故收藏也按名归属，前后端统一。
 */
export const useFavoritesStore = defineStore('favorites', () => {
    // 维度类型 → 已收藏取值名集合（重新赋值整对象以触发响应式）
    const byType = ref<Record<string, Set<string>>>({})

    async function load(entityType: string) {
        try {
            const names = await invoke<string[]>('list_favorites', { entityType })
            byType.value = { ...byType.value, [entityType]: new Set(names) }
        } catch (e) {
            console.error('加载收藏失败:', e)
        }
    }

    function isFavorite(entityType: string, name: string): boolean {
        return byType.value[entityType]?.has(name) ?? false
    }

    function favoriteSet(entityType: string): Set<string> {
        return byType.value[entityType] ?? new Set<string>()
    }

    async function toggle(entityType: string, name: string): Promise<boolean> {
        const fav = await invoke<boolean>('toggle_favorite', { entityType, name })
        const set = new Set(byType.value[entityType] ?? [])
        if (fav) set.add(name)
        else set.delete(name)
        byType.value = { ...byType.value, [entityType]: set }
        return fav
    }

    return { byType, load, isFavorite, favoriteSet, toggle }
})
