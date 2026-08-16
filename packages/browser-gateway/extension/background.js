// DSH++ Browser Control - background service worker.
// Bridges Native Messaging (the DSH++ gateway) to page tabs:
//   gateway <-> native host <-> this worker <-> content script (page)
// All requests arrive as { id, action, payload } and are answered with
// { id, ok, result | error }.
//
// MV3 service workers are event-driven: the worker only starts when a
// registered browser event fires. We register several always-on listeners
// (onInstalled / onStartup / tab events / runtime messages) so the worker
// wakes up, connects the native host, and stays alive while the port is open.

const HOST_NAME = 'com.dshplusplus.browser'
let port = null
let connecting = false

function connectNative() {
  if (port !== null || connecting) return
  connecting = true
  try {
    port = chrome.runtime.connectNative(HOST_NAME)
    connecting = false
  } catch (error) {
    connecting = false
    console.error('connectNative failed', error)
    setTimeout(connectNative, 2000)
    return
  }
  port.onMessage.addListener((message) => {
    handleRequest(message).then((response) => {
      try {
        port.postMessage(response)
      } catch (error) {
        console.error('postMessage failed', error)
      }
    })
  })
  port.onDisconnect.addListener(() => {
    port = null
    const error = chrome.runtime.lastError
    if (error && String(error.message).includes('Host not found')) {
      console.error('native host not registered: com.dshplusplus.browser')
    }
    setTimeout(connectNative, 2000)
  })
}

async function handleRequest(message) {
  const id = String(message.id ?? '')
  const action = String(message.action ?? '')
  const payload = message.payload ?? {}
  try {
    const result = await dispatch(action, payload)
    return { id, ok: true, result }
  } catch (error) {
    return { id, ok: false, error: String(error && error.message ? error.message : error) }
  }
}

async function dispatch(action, payload) {
  switch (action) {
    case 'open': {
      const tab = await chrome.tabs.create({ url: String(payload.url) })
      // Wait briefly for navigation so the reply carries a real url/title.
      const deadline = Date.now() + 4000
      let current = tab
      while (Date.now() < deadline) {
        const fresh = await chrome.tabs.get(tab.id).catch(() => undefined)
        if (fresh && fresh.status === 'complete' && fresh.url && fresh.url !== 'about:blank') {
          current = fresh
          break
        }
        await new Promise((resolve) => setTimeout(resolve, 200))
      }
      return { tabId: current.id, url: current.url, title: current.title }
    }
    case 'list_tabs': {
      const tabs = await chrome.tabs.query({})
      return {
        tabs: tabs
          .filter((tab) => tab.id !== undefined && tab.url && tab.url.startsWith('http'))
          .map((tab) => ({ tabId: tab.id, title: tab.title, url: tab.url, active: tab.active, windowId: tab.windowId })),
      }
    }
    case 'close_tab': {
      if (payload.tabId !== undefined) {
        await chrome.tabs.remove(Number(payload.tabId))
      } else {
        const [active] = await chrome.tabs.query({ active: true, currentWindow: true })
        if (active && active.id !== undefined) await chrome.tabs.remove(active.id)
      }
      return { ok: true }
    }
    case 'observe': {
      const tab = await resolveTab(payload.tabId)
      const response = await sendToTab(tab, { action: 'observe' })
      let screenshot = null
      try {
        const dataUrl = await chrome.tabs.captureVisibleTab(tab.windowId, { format: 'png' })
        screenshot = dataUrl
      } catch {
        screenshot = null
      }
      return { ...response, tabId: tab.id, url: tab.url, screenshot }
    }
    case 'click':
    case 'type':
    case 'press': {
      const tab = await resolveTab(payload.tabId)
      const response = await sendToTab(tab, { action, payload })
      return { ...response, tabId: tab.id }
    }
    case 'evaluate': {
      // 任意 JS 执行：content script / executeScript 的 eval 都受页面 CSP
      // 的 unsafe-eval 限制（Bing/Google 等站点会拦截）。chrome.debugger 的
      // Runtime.evaluate 走 CDP 在浏览器进程层面执行，不受页面 CSP 限制
      // （与受管 Chrome 模式、Playwright/Tabbit 的实现一致）。附加调试器
      // 时浏览器会短暂显示调试提示条。
      const tab = await resolveTab(payload.tabId)
      const expression = String(payload.expression ?? '')
      const debuggee = { tabId: tab.id }
      await chrome.debugger.attach(debuggee, '1.3')
      try {
        const response = await chrome.debugger.sendCommand(debuggee, 'Runtime.evaluate', {
          expression,
          returnByValue: true,
          awaitPromise: true,
        })
        if (response.exceptionDetails) {
          const detail = response.exceptionDetails.exception
            ? response.exceptionDetails.exception.description || response.exceptionDetails.exception.value
            : response.exceptionDetails.text
          throw new Error(`页面执行异常: ${String(detail).slice(0, 300)}`)
        }
        const value = response.result && response.result.value !== undefined
          ? response.result.value
          : response.result && response.result.description !== undefined
            ? response.result.description
            : null
        return { ok: true, result: value, tabId: tab.id }
      } finally {
        await chrome.debugger.detach(debuggee).catch(() => undefined)
      }
    }
    default:
      throw new Error(`unknown action: ${action}`)
  }
}

async function resolveTab(tabId) {
  if (tabId !== undefined) {
    return chrome.tabs.get(Number(tabId))
  }
  const [active] = await chrome.tabs.query({ active: true, currentWindow: true })
  if (!active) throw new Error('no active tab')
  return active
}

async function sendToTab(tab, message) {
  if (tab.id === undefined) throw new Error('invalid tab')
  try {
    return await chrome.tabs.sendMessage(tab.id, message)
  } catch (error) {
    throw new Error(
      `cannot talk to tab ${tab.id}; press F5 on that page, or check chrome://extensions that DSH++ Browser Control is enabled`,
    )
  }
}

// Event-driven wakeups: any of these starts the worker, which then keeps the
// native messaging port open (an open port keeps the worker alive).
chrome.runtime.onInstalled.addListener(() => connectNative())
chrome.runtime.onStartup.addListener(() => connectNative())
chrome.tabs.onActivated.addListener(() => connectNative())
chrome.tabs.onUpdated.addListener(() => connectNative())
chrome.runtime.onMessage.addListener((_message, _sender, _sendResponse) => {
  connectNative()
  return false
})

// Keepalive: MV3 service workers idle-terminate after ~30s even with a
// native messaging port open, which silently breaks the bridge. A minimum
// 30s alarm wakes the worker, which reconnects the port. The alarm listener
// also proves to Chrome that this worker must be started periodically.
chrome.alarms.create('dshplusplus-keepalive', { periodInMinutes: 0.5 })
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === 'dshplusplus-keepalive') connectNative()
})

// Immediate attempt when the worker starts for any reason.
connectNative()
