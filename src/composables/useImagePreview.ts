/**
 * 统一图片预览 composable
 * 基于 Fancybox 实现，支持自定义操作按钮
 */
import { Fancybox } from '@fancyapps/ui/dist/fancybox/'
import '@fancyapps/ui/dist/fancybox/fancybox.css'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { toast } from 'vue-sonner'

/** 图片项定义 */
export interface PreviewImage {
  src: string
  /** 图片标题/说明 */
  title?: string
  /** 是否有关联的本地视频（决定是否显示删除按钮） */
  hasLocalVideo?: boolean
  /** 另存为时使用的原始来源（http/data/本地路径）；缺省时从 src 还原 */
  downloadSrc?: string
  /** 自定义数据，回调时原样返回 */
  data?: Record<string, any>
}

/** 回调事件 */
export interface PreviewCallbacks {
  /** 删除图片 */
  onDelete?: (image: PreviewImage, index: number) => void | Promise<void>
}

/** 按钮 data 属性名，用于 DOM 查询 */
const BTN_ATTR = 'data-fb-action'

/** Fancybox 是否正在打开 */
let fancyboxOpen = false

/** 检查 Fancybox 是否正在打开（供外部判断是否阻止 interact-outside） */
export function isFancyboxOpen() {
  return fancyboxOpen
}

/**
 * 把预览用的 src 还原成后端可保存的来源。
 * 本地图片经 convertFileSrc 变成 `http://asset.localhost/<encoded>`（Win）或
 * `asset://localhost/<encoded>`（mac/linux），需解码回真实文件路径；
 * 远程 http(s) / data URL 原样返回（后端负责下载或解码）。
 */
function toSavableSource(src: string): string {
  const assetMatch = src.match(
    /^(?:https?:\/\/asset\.localhost\/|asset:\/\/localhost\/|asset:\/\/)(.+)$/i,
  )
  if (assetMatch) {
    const encoded = assetMatch[1].split(/[?#]/)[0]
    try {
      return decodeURIComponent(encoded)
    } catch {
      return encoded
    }
  }
  return src
}

/** 从来源与标题推断另存为对话框的默认文件名 */
function deriveDefaultName(savable: string, title?: string): string {
  if (savable.startsWith('data:')) {
    const header = savable.slice(5, savable.indexOf(',') === -1 ? undefined : savable.indexOf(','))
    const mime = header.split(';')[0]
    const ext = (mime.split('/')[1] || 'jpg').split('+')[0]
    return `${title?.trim() || 'image'}.${ext}`
  }
  const clean = savable.split(/[?#]/)[0]
  let base = clean.split(/[\\/]/).pop() || ''
  // 远程 URL 的文件名可能是 percent-encoded；本地路径已是真实名，解码失败则保留原样
  try {
    base = decodeURIComponent(base)
  } catch {
    /* 保留原始文件名 */
  }
  if (base && /\.[a-z0-9]{2,5}$/i.test(base)) return base
  return `${title?.trim() || base || 'image'}.jpg`
}

/**
 * 根据当前幻灯片的图片属性，显示/隐藏自定义按钮
 */
function syncButtonVisibility(images: PreviewImage[], index: number) {
  const img = images[index]
  if (!img) return

  document.querySelectorAll<HTMLElement>(`[${BTN_ATTR}]`).forEach((el) => {
    const action = el.getAttribute(BTN_ATTR)
    let visible = false

    switch (action) {
      case 'delete':
        visible = !!img.hasLocalVideo
        break
      case 'download':
        visible = true
        break
    }

    el.style.display = visible ? '' : 'none'
  })
}


/**
 * 打开图片预览
 */
export function openImagePreview(
  images: PreviewImage[],
  startIndex = 0,
  callbacks: PreviewCallbacks = {}
) {
  if (!images.length) return

  const slides = images.map((img) => ({
    src: img.src,
    caption: img.title || '',
  }))

  const toolbarItems: Record<string, any> = {
    deleteBtn: {
      tpl: `<button class="f-button" ${BTN_ATTR}="delete" title="删除图片">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
        </svg>
      </button>`,
      click: () => {
        const fb = Fancybox.getInstance()
        if (!fb) return
        const idx = fb.getSlide()?.index ?? 0
        const img = images[idx]
        if (img?.hasLocalVideo && callbacks.onDelete) {
          callbacks.onDelete(img, idx)
          fb.close()
        }
      },
    },
    downloadBtn: {
      tpl: `<button class="f-button" ${BTN_ATTR}="download" title="另存为">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
        </svg>
      </button>`,
      click: async () => {
        const fb = Fancybox.getInstance()
        if (!fb) return
        const idx = fb.getSlide()?.index ?? 0
        const img = images[idx]
        if (!img) return

        const savable = toSavableSource(img.downloadSrc ?? img.src)
        const defaultName = deriveDefaultName(savable, img.title)

        try {
          const targetPath = await save({
            defaultPath: defaultName,
            filters: [
              { name: '图片', extensions: ['jpg', 'jpeg', 'png', 'webp', 'gif', 'bmp'] },
            ],
          })
          if (!targetPath) return

          await invoke('save_image_as', { src: savable, targetPath })
          toast.success('图片已保存')
        } catch (e) {
          toast.error('保存失败: ' + String(e))
        }
      },
    },
  }

  Fancybox.show(slides as any, {
    startIndex,
    Carousel: {
      Toolbar: {
        items: toolbarItems,
        display: {
          left: ['counter'],
          middle: [
            'zoomIn',
            'zoomOut',
            'toggle1to1',
            'fullscreen',
          ],
          right: [
            'autoplay',
            'thumbs',
            'deleteBtn',
            'downloadBtn',
            'close',
          ],
        },
      },
    },
    on: {
      ready: () => {
        fancyboxOpen = true
        syncButtonVisibility(images, startIndex)
      },
      close: () => {
        fancyboxOpen = false
      },
      'Carousel.change': () => {
        const fb = Fancybox.getInstance()
        if (!fb) return
        const idx = fb.getSlide()?.index ?? 0
        syncButtonVisibility(images, idx)
      },
    },
  })
}


/**
 * 打开长截图预览（单张，使用 Fancybox）
 * 先用 Image 探测图片是否可加载，成功则打开 Fancybox，失败调用 onError 回调
 */
export function openLongScreenshot(
  url: string,
  title = '长截图',
  onError?: () => void
) {
  const img = new Image()
  img.onload = () => {
    openImagePreview([{ src: url, title }], 0)
  }
  img.onerror = () => {
    if (onError) onError()
  }
  img.src = url
}

/**
 * 销毁所有 Fancybox 实例
 */
export function destroyImagePreview() {
  Fancybox.close()
}
