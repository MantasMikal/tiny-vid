import type { TitlebarMenu } from '@/app/components/window/TitlebarMenu'

export const menuItems: TitlebarMenu[] = [
  {
    name: 'File',
    items: [
      {
        name: 'Exit',
        action: 'window-close',
      },
    ],
  },
  {
    name: 'View',
    items: [
      {
        name: 'Dark Mode',
        action: 'window-darkmode-toggle',
        shortcut: 'Toggle',
        actionCallback: () => {
          document.documentElement.classList.toggle('dark')
        },
      },
      {
        name: '---',
      },
      {
        name: 'Reload',
        action: 'web-reload',
        shortcut: 'Ctrl+R',
      },
      {
        name: 'Force Reload',
        action: 'web-force-reload',
        shortcut: 'Ctrl+Shift+R',
      },
      {
        name: 'Toggle Developer Tools',
        action: 'web-toggle-devtools',
        shortcut: 'Ctrl+Shift+I',
      },
      {
        name: '---',
      },
      {
        name: 'Actual Size',
        action: 'web-actual-size',
        shortcut: 'Ctrl+0',
      },
      {
        name: 'Zoom In',
        action: 'web-zoom-in',
        shortcut: 'Ctrl++',
      },
      {
        name: 'Zoom Out',
        action: 'web-zoom-out',
        shortcut: 'Ctrl+-',
      },
      {
        name: '---',
      },
      {
        name: 'Toggle Fullscreen',
        action: 'web-toggle-fullscreen',
        shortcut: 'F11',
      },
    ],
  },
  {
    name: 'Window',
    items: [
      {
        name: 'Maximize',
        action: 'window-maximize-toggle',
        shortcut: 'Toggle',
      },
      {
        name: 'Minimize',
        action: 'window-minimize',
        shortcut: 'Ctrl+M',
      },
      {
        name: 'Close',
        action: 'window-close',
        shortcut: 'Ctrl+W',
      },
    ],
  },
]
