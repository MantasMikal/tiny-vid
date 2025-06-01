import { BrowserWindow, shell, app, protocol, net } from 'electron'
import { join } from 'path'
import { existsSync } from 'fs'
import { registerWindowIPC } from '@/lib/window/ipcEvents'
import appIcon from '@/resources/build/icon.png?asset'
import { pathToFileURL } from 'url'

export function createAppWindow(): void {
  // Register custom protocols
  registerResourcesProtocol()
  registerLocalFileProtocol()

  // Create the main window.
  const mainWindow = new BrowserWindow({
    width: 900,
    height: 670,
    show: false,
    backgroundColor: '#1c1c1c',
    icon: appIcon,
    frame: false,
    titleBarStyle: 'hiddenInset',
    title: 'Electron React App',
    maximizable: false,
    resizable: false,
    webPreferences: {
      preload: join(__dirname, '../preload/preload.js'),
      sandbox: false,
      webSecurity: true,
    },
  })

  // Register IPC events for the main window.
  registerWindowIPC(mainWindow)

  mainWindow.on('ready-to-show', () => {
    mainWindow.show()
  })

  mainWindow.webContents.setWindowOpenHandler((details) => {
    shell.openExternal(details.url)
    return { action: 'deny' }
  })

  // HMR for renderer base on electron-vite cli.
  // Load the remote URL for development or the local html file for production.
  if (!app.isPackaged && process.env['ELECTRON_RENDERER_URL']) {
    mainWindow.loadURL(process.env['ELECTRON_RENDERER_URL'])
  } else {
    mainWindow.loadFile(join(__dirname, '../renderer/index.html'))
  }
}

// Register custom protocol for assets
function registerResourcesProtocol() {
  protocol.handle('res', async (request) => {
    try {
      const url = new URL(request.url)
      // Combine hostname and pathname to get the full path
      const fullPath = join(url.hostname, url.pathname.slice(1))
      const filePath = join(__dirname, '../../resources', fullPath)
      return net.fetch(pathToFileURL(filePath).toString())
    } catch (error) {
      console.error('Protocol error:', error)
      return new Response('Resource not found', { status: 404 })
    }
  })
}

// Register custom protocol for local files
function registerLocalFileProtocol() {
  protocol.handle('local-file', async (request) => {
    try {
      const url = new URL(request.url)
      // The path is the hostname (we use hostname to avoid path encoding issues)
      const filePath = decodeURIComponent(url.hostname)

      if (!existsSync(filePath)) {
        console.error('File does not exist:', filePath)
        return new Response('File not found', { status: 404 })
      }

      const response = await net.fetch(pathToFileURL(filePath).toString())

      // Create a new response with the correct headers
      const headers = new Headers(response.headers)
      // Set content type based on file extension
      const ext = filePath.split('.').pop()?.toLowerCase()
      const mimeTypes: Record<string, string> = {
        mp4: 'video/mp4',
        webm: 'video/webm',
        mov: 'video/quicktime',
        mpeg: 'video/mpeg',
        '3gp': 'video/3gpp',
        avi: 'video/x-msvideo',
        flv: 'video/x-flv',
        mkv: 'video/x-matroska',
        ogg: 'video/ogg',
      }
      if (ext && mimeTypes[ext]) {
        headers.set('content-type', mimeTypes[ext])
      }

      // Add CORS headers
      headers.set('Access-Control-Allow-Origin', '*')
      headers.set('Access-Control-Allow-Methods', 'GET, OPTIONS')
      headers.set('Access-Control-Allow-Headers', 'Content-Type')

      return new Response(response.body, {
        status: response.status,
        statusText: response.statusText,
        headers,
      })
    } catch (error) {
      console.error('Local file protocol error:', error)
      return new Response('File not found', { status: 404 })
    }
  })
}
