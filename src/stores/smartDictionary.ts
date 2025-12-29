/**
 * 智能词典 Store
 * 支持手动添加词条，在转录和校正时自动应用
 */
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import logger from '@/utils/logger'

export interface DictionaryEntry {
  id: string
  correct: string      // 正确写法
  variants: string[]   // 已知的错误变体
  useCount: number     // 使用次数
  createdAt: number
  lastUsedAt: number
}

export const useSmartDictionaryStore = defineStore('smartDictionary', () => {
  const entries = ref<DictionaryEntry[]>([])
  const totalCount = computed(() => entries.value.length)

  // ========== 添加词条 ==========
  const addManual = (correct: string, variants: string[] = []): { entry: DictionaryEntry; merged: boolean; newVariants: string[] } | null => {
    if (!correct.trim()) return null

    // 检查是否已存在
    const existing = entries.value.find(e => e.correct === correct)
    if (existing) {
      // 合并变体，记录新增的变体
      const newVariants: string[] = []
      for (const v of variants) {
        if (v && !existing.variants.includes(v)) {
          existing.variants.push(v)
          newVariants.push(v)
        }
      }
      save()
      return { entry: existing, merged: true, newVariants }
    }

    const entry: DictionaryEntry = {
      id: `dict_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
      correct: correct.trim(),
      variants: variants.filter(v => v.trim()),
      useCount: 0,
      createdAt: Date.now(),
      lastUsedAt: Date.now(),
    }

    entries.value.push(entry)
    save()
    logger.info('添加词典条目', { correct, variants })
    return { entry, merged: false, newVariants: entry.variants }
  }

  // ========== 检查文本是否匹配词典（不执行替换） ==========
  const matchesDictionary = (text: string): boolean => {
    for (const entry of entries.value) {
      for (const variant of entry.variants) {
        if (text.includes(variant)) {
          return true
        }
      }
    }
    return false
  }

  // ========== 应用词典 ==========
  const applyDictionary = (text: string): { result: string; replacements: Array<{ from: string; to: string }> } => {
    let result = text
    const replacements: Array<{ from: string; to: string }> = []

    for (const entry of entries.value) {
      for (const variant of entry.variants) {
        if (result.includes(variant)) {
          result = result.replaceAll(variant, entry.correct)
          replacements.push({ from: variant, to: entry.correct })
          entry.useCount++
          entry.lastUsedAt = Date.now()
        }
      }
    }

    if (replacements.length > 0) {
      save()
      logger.info('应用词典替换', { replacements })
    }

    return { result, replacements }
  }

  // ========== 管理操作 ==========
  const removeEntry = (id: string) => {
    const index = entries.value.findIndex(e => e.id === id)
    if (index !== -1) {
      const removed = entries.value.splice(index, 1)[0]
      save()
      logger.info('删除词典条目', { correct: removed?.correct })
    }
  }

  const addVariant = (id: string, variant: string) => {
    const entry = entries.value.find(e => e.id === id)
    if (entry && variant.trim() && !entry.variants.includes(variant.trim())) {
      entry.variants.push(variant.trim())
      save()
    }
  }

  // ========== 修改正确写法 ==========
  const updateCorrect = (id: string, newCorrect: string): boolean => {
    const trimmed = newCorrect.trim()
    if (!trimmed) return false
    
    const entry = entries.value.find(e => e.id === id)
    if (!entry) return false
    
    // 检查是否与其他词条重复
    const duplicate = entries.value.find(e => e.id !== id && e.correct === trimmed)
    if (duplicate) return false
    
    entry.correct = trimmed
    save()
    logger.info('修改词典正确写法', { id, newCorrect: trimmed })
    return true
  }

  // ========== 修改变体 ==========
  const updateVariant = (id: string, oldVariant: string, newVariant: string): boolean => {
    const trimmed = newVariant.trim()
    if (!trimmed) return false
    
    const entry = entries.value.find(e => e.id === id)
    if (!entry) return false
    
    const index = entry.variants.indexOf(oldVariant)
    if (index === -1) return false
    
    // 检查是否重复
    if (entry.variants.includes(trimmed) && oldVariant !== trimmed) return false
    
    entry.variants[index] = trimmed
    save()
    logger.info('修改词典变体', { id, oldVariant, newVariant: trimmed })
    return true
  }

  const removeVariant = (id: string, variant: string) => {
    const entry = entries.value.find(e => e.id === id)
    if (entry) {
      const index = entry.variants.indexOf(variant)
      if (index !== -1) {
        entry.variants.splice(index, 1)
        save()
      }
    }
  }

  const clearAll = () => {
    entries.value = []
    save()
    logger.info('清空词典')
  }

  // ========== 持久化 ==========
  const save = () => {
    try {
      localStorage.setItem('smart-dictionary', JSON.stringify(entries.value))
    } catch (error) {
      logger.error('保存词典失败', { error: String(error) })
    }
  }

  const load = () => {
    try {
      const saved = localStorage.getItem('smart-dictionary')
      if (saved) {
        entries.value = JSON.parse(saved)
        logger.debug('加载词典', { count: entries.value.length })
      }
    } catch (error) {
      logger.error('加载词典失败', { error: String(error) })
    }
  }

  // 导出词典
  const exportDictionary = (): string => {
    return JSON.stringify(entries.value, null, 2)
  }

  // 导入词典
  const importDictionary = (json: string) => {
    try {
      const imported = JSON.parse(json) as DictionaryEntry[]
      if (!Array.isArray(imported)) throw new Error('Invalid format')

      for (const item of imported) {
        const existing = entries.value.find(e => e.correct === item.correct)
        if (existing) {
          for (const v of item.variants) {
            if (!existing.variants.includes(v)) {
              existing.variants.push(v)
            }
          }
        } else {
          entries.value.push({
            ...item,
            id: `imported_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`
          })
        }
      }

      save()
      logger.info('导入词典', { count: imported.length })
      return true
    } catch (error) {
      logger.error('导入词典失败', { error: String(error) })
      return false
    }
  }

  // 初始化
  load()

  return {
    entries,
    totalCount,
    addManual,
    removeEntry,
    addVariant,
    updateCorrect,
    updateVariant,
    removeVariant,
    clearAll,
    matchesDictionary,
    applyDictionary,
    exportDictionary,
    importDictionary,
  }
})
