// DSH++ Browser Control - content script (runs inside pages).
// Handles observe / click / type / press requests from the background worker.

function collectElements() {
  return [...document.querySelectorAll('a,button,input,textarea,select,[role="button"],[role="link"],[onclick]')]
    .slice(0, 200)
    .map((el, index) => ({
      index,
      tag: el.tagName.toLowerCase(),
      text: (el.innerText || el.value || el.placeholder || el.getAttribute('aria-label') || '').trim().slice(0, 80),
      href: el.tagName === 'A' && el.href ? el.href.slice(0, 200) : null,
      type: el.getAttribute('type'),
    }))
}

function snapshot() {
  return {
    title: document.title,
    url: location.href,
    text: (document.body ? document.body.innerText : '').slice(0, 20000),
    elements: collectElements(),
  }
}

function findElement(target) {
  if (target.ref !== undefined) {
    return collectElements()[Number(target.ref)]
  }
  if (target.selector) {
    return document.querySelector(String(target.selector))
  }
  return null
}

function dispatchInputEvents(el) {
  el.dispatchEvent(new Event('input', { bubbles: true }))
  el.dispatchEvent(new Event('change', { bubbles: true }))
}

function handleAction(action, payload) {
  switch (action) {
    case 'observe':
      return { ok: true, result: snapshot() }
    case 'click': {
      const el = findElement(payload)
      if (!el) return { ok: false, error: 'element not found' }
      el.scrollIntoView({ block: 'center' })
      el.click()
      return { ok: true }
    }
    case 'type': {
      const el = findElement(payload)
      if (!el) return { ok: false, error: 'element not found' }
      if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el.isContentEditable)) {
        return { ok: false, error: `target is not an input element (tag=${el.tagName})` }
      }
      el.focus()
      const prototype = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype
      const setter = prototype && Object.getOwnPropertyDescriptor(prototype, 'value')?.set
      if (setter && !el.isContentEditable) setter.call(el, String(payload.text ?? ''))
      else el.textContent = String(payload.text ?? '')
      dispatchInputEvents(el)
      return { ok: true }
    }
    case 'press': {
      const key = String(payload.key ?? '')
      const options = { key, bubbles: true, cancelable: true }
      const keyMap = {
        Enter: { code: 'Enter', keyCode: 13 },
        Escape: { code: 'Escape', keyCode: 27 },
        Tab: { code: 'Tab', keyCode: 9 },
        Backspace: { code: 'Backspace', keyCode: 8 },
        ArrowUp: { code: 'ArrowUp', keyCode: 38 },
        ArrowDown: { code: 'ArrowDown', keyCode: 40 },
        ArrowLeft: { code: 'ArrowLeft', keyCode: 37 },
        ArrowRight: { code: 'ArrowRight', keyCode: 39 },
        Home: { code: 'Home', keyCode: 36 },
        End: { code: 'End', keyCode: 35 },
        PageUp: { code: 'PageUp', keyCode: 33 },
        PageDown: { code: 'PageDown', keyCode: 34 },
      }
      const details = keyMap[key] ?? { code: key, keyCode: key.length === 1 ? key.toUpperCase().charCodeAt(0) : 0 }
      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keydown', { ...options, code: details.code, keyCode: details.keyCode, which: details.keyCode }),
      )
      document.activeElement?.dispatchEvent(
        new KeyboardEvent('keyup', { ...options, code: details.code, keyCode: details.keyCode, which: details.keyCode }),
      )
      return { ok: true }
    }
    case 'evaluate': {
      // 进阶能力：执行任意 JS（Tabbit/Playwright 式）。页面作用域内可访问
      // window/document；async IIFE 由调用方包装，这里 await 结果。
      return (async () => {
        const expression = String(payload.expression ?? '')
        if (!expression.trim()) return { ok: false, error: 'expression 不能为空' }
        const value = await (0, eval)(expression)
        return { ok: true, result: value }
      })()
    }
    default:
      return { ok: false, error: `unknown action: ${action}` }
  }
}

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (!message || typeof message.action !== 'string') return
  const result = handleAction(message.action, message.payload ?? {})
  // evaluate 返回 Promise：保持消息通道，异步回复（return true）。
  if (result && typeof result.then === 'function') {
    result.then(sendResponse).catch((error) => sendResponse({ ok: false, error: String(error) }))
    return true
  }
  sendResponse(result)
  return false
})
