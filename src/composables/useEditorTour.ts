import { driver } from 'driver.js'
import 'driver.js/dist/driver.css'
import { useConfigStore } from '@/stores/config'
import { useAudioStore } from '@/stores/audio'

export function useEditorTour() {
  const configStore = useConfigStore()
  const audioStore = useAudioStore()

  const startTour = () => {
    const hasAudio = audioStore.currentAudio !== null

    // 根据是否有音频动态生成步骤
    const steps = [
      {
        element: '.editor-sidebar',
        popover: {
          title: '工具栏',
          description: '新增、搜索、分割、合并字幕等常用操作都在这里',
          side: 'right' as const,
          align: 'start' as const,
        },
      },
      {
        element: '.subtitle-list-panel',
        popover: {
          title: '字幕列表',
          description: '所有字幕条目都在这里，点击选中，双击快速编辑文本',
          side: 'right' as const,
          align: 'center' as const,
        },
      },
      {
        element: '.subtitle-edit-panel',
        popover: {
          title: '编辑面板',
          description: '在这里编辑字幕文本、调整开始和结束时间',
          side: 'left' as const,
          align: 'start' as const,
        },
      },
    ]

    // 根据是否有音频显示不同的引导
    if (hasAudio) {
      steps.push({
        element: '.waveform-container',
        popover: {
          title: '波形时间轴',
          description: '在波形上直接拖拽字幕块调整时间，滚轮缩放波形',
          side: 'top' as const,
          align: 'center' as const,
        },
      } as any)
    } else {
      steps.push({
        element: '.audio-empty-state',
        popover: {
          title: '加载音频',
          description: '点击「选择音频文件」导入音频，即可使用波形编辑功能',
          side: 'bottom' as const,
          align: 'center' as const,
        },
      } as any)
    }

    // 最后一步
    steps.push({
      popover: {
        title: '开始使用吧！',
        description: '按 Ctrl+, (Mac: ⌘+,) 打开设置查看快捷键列表。如需重新查看引导，可在设置中找到。',
      },
    } as any)

    const driverObj = driver({
      showProgress: true,
      animate: true,
      allowClose: true,
      overlayColor: 'rgba(0, 0, 0, 0.6)',
      stagePadding: 8,
      stageRadius: 8,
      popoverClass: 'vosub-tour-popover',
      progressText: '{{current}} / {{total}}',
      nextBtnText: '下一步',
      prevBtnText: '上一步',
      doneBtnText: '完成',
      onDestroyStarted: () => {
        configStore.markEditorTourCompleted()
        driverObj.destroy()
      },
      steps,
    })

    driverObj.drive()
  }

  const shouldShowTour = () => {
    return !configStore.onboardingState.editorTourCompleted
  }

  return {
    startTour,
    shouldShowTour,
  }
}
