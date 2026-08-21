/** Content-script evaluation fragment shared by managed CDP and shared-tab modes. */
export const PAGE_SNAPSHOT_SCRIPT = `(() => {
  const elements = [...document.querySelectorAll('a,button,input,textarea,select,[role="button"],[role="link"],[onclick]')]
    .slice(0, 200)
    .map((el, index) => ({
      index,
      tag: el.tagName.toLowerCase(),
      text: (el.innerText || el.value || el.placeholder || el.getAttribute('aria-label') || '').trim().slice(0, 80),
      href: el.tagName === 'A' && el.href ? el.href.slice(0, 200) : null,
      type: el.getAttribute('type'),
    }));
  return {
    title: document.title,
    url: location.href,
    text: (document.body ? document.body.innerText : '').slice(0, 20000),
    elements,
  };
})()`
