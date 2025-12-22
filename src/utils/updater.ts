/**
 * 应用更新检测模块
 * 从 GitHub Releases 检测新版本并提供下载
 */

import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-shell'

// GitHub 仓库信息
const GITHUB_OWNER = '1017416696'
const GITHUB_REPO = 'VoSub'

// GitHub API 镜像列表（可添加国内镜像）
const GITHUB_MIRRORS = [
  `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest`,
  // 国内镜像（如果有的话可以添加）
]

export interface ReleaseInfo {
  version: string
  name: string
  body: string // Release notes (markdown)
  publishedAt: string
  htmlUrl: string
  assets: ReleaseAsset[]
}

export interface ReleaseAsset {
  name: string
  size: number
  downloadUrl: string
  browserDownloadUrl: string
}

export interface UpdateCheckResult {
  hasUpdate: boolean
  currentVersion: string
  latestVersion?: string
  releaseInfo?: ReleaseInfo
  error?: string
}

/**
 * 获取当前应用版本
 */
export async function getCurrentVersion(): Promise<string> {
  try {
    const version = await invoke<string>('get_app_version')
    return version
  } catch {
    // 如果命令不存在，返回默认版本
    return '1.0.0'
  }
}

/**
 * 比较版本号
 * @returns 1 if v1 > v2, -1 if v1 < v2, 0 if equal
 */
export function compareVersions(v1: string, v2: string): number {
  const parts1 = v1.replace(/^v/, '').split('.').map(Number)
  const parts2 = v2.replace(/^v/, '').split('.').map(Number)

  for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
    const p1 = parts1[i] || 0
    const p2 = parts2[i] || 0
    if (p1 > p2) return 1
    if (p1 < p2) return -1
  }
  return 0
}

/**
 * 从 GitHub 获取最新 Release 信息
 */
export async function fetchLatestRelease(): Promise<ReleaseInfo | null> {
  for (const apiUrl of GITHUB_MIRRORS) {
    try {
      const response = await fetch(apiUrl, {
        headers: {
          Accept: 'application/vnd.github.v3+json',
          'User-Agent': 'VoSub-Updater',
        },
      })

      if (!response.ok) {
        console.warn(`GitHub API 请求失败: ${response.status}`)
        continue
      }

      const data = await response.json()

      // 解析 assets
      const assets: ReleaseAsset[] = (data.assets || []).map((asset: any) => ({
        name: asset.name,
        size: asset.size,
        downloadUrl: asset.url,
        browserDownloadUrl: asset.browser_download_url,
      }))

      return {
        version: data.tag_name?.replace(/^v/, '') || data.name,
        name: data.name,
        body: data.body || '',
        publishedAt: data.published_at,
        htmlUrl: data.html_url,
        assets,
      }
    } catch (error) {
      console.warn(`从 ${apiUrl} 获取更新信息失败:`, error)
      continue
    }
  }

  return null
}

/**
 * 检查更新
 */
export async function checkForUpdates(): Promise<UpdateCheckResult> {
  try {
    const currentVersion = await getCurrentVersion()
    const releaseInfo = await fetchLatestRelease()

    if (!releaseInfo) {
      return {
        hasUpdate: false,
        currentVersion,
        error: '无法连接到更新服务器，请检查网络连接',
      }
    }

    const hasUpdate = compareVersions(releaseInfo.version, currentVersion) > 0

    return {
      hasUpdate,
      currentVersion,
      latestVersion: releaseInfo.version,
      releaseInfo,
    }
  } catch (error) {
    return {
      hasUpdate: false,
      currentVersion: '1.0.0',
      error: error instanceof Error ? error.message : '检查更新时发生错误',
    }
  }
}

/**
 * 获取当前平台对应的下载资源
 */
export function getPlatformAsset(assets: ReleaseAsset[]): ReleaseAsset | null {
  const platform = navigator.platform.toLowerCase()

  let patterns: string[] = []

  if (platform.includes('mac')) {
    // macOS: 优先 .dmg，其次 .app.tar.gz
    patterns = ['.dmg', '.app.tar.gz', 'darwin', 'macos']
  } else if (platform.includes('win')) {
    // Windows: 优先 .msi，其次 .exe
    patterns = ['.msi', '.exe', '-setup', 'windows']
  } else {
    // Linux: 优先 .AppImage，其次 .deb
    patterns = ['.AppImage', '.deb', '.rpm', 'linux']
  }

  for (const pattern of patterns) {
    const asset = assets.find((a) => a.name.toLowerCase().includes(pattern.toLowerCase()))
    if (asset) return asset
  }

  return assets[0] || null
}

/**
 * 在浏览器中打开下载链接
 */
export async function openDownloadPage(url: string): Promise<void> {
  try {
    await open(url)
  } catch (error) {
    // 如果 shell.open 失败，尝试使用 window.open
    window.open(url, '_blank')
  }
}

/**
 * 格式化文件大小
 */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
