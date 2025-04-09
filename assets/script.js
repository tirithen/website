async function navigate(url) {
    try {
        const response = await fetch(url, {
            headers: {
                'Accept': 'text/html; partial=true'
            }
        });

        if (!response.ok) {
            throw new Error('fetch failed');
        }

        const html = await response.text();
        const parser = new DOMParser();
        const doc = parser.parseFromString(html, 'text/html');
        const newContent = doc.querySelector('main').innerHTML;
        const newTitle = doc.title;

        const transition = document.startViewTransition(async () => {
            document.title = newTitle;
            document.querySelector('main').innerHTML = newContent;
            history.pushState({}, '', url);
        });
        await transition.finished;
    } catch (error) {
        window.location.href = url;
    }
}

document.addEventListener('click', event => {
  const link = event.closest('a[href^="/"]');
  if (!link || link.origin !== location.origin) {
    return;
  }

  event.preventDefault();
  window.history.pushState(undefined, undefined, link.href);
  window.dispatchEvent(new PopStateEvent());
});
