const CACHE_NAME = 'tct-pwa-v2';
const STATIC_ASSETS = ['/manifest.webmanifest', '/tct-pwa.js', '/icon.svg'];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS))
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter((key) => key !== CACHE_NAME)
          .map((key) => caches.delete(key))
      )
    )
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  // Never intercept WebSocket/token endpoints or OAuth2 proxy flows
  if (
    url.pathname.startsWith('/ws') ||
    url.pathname.startsWith('/token') ||
    url.pathname.startsWith('/oauth2')
  ) {
    return;
  }
  if (STATIC_ASSETS.includes(url.pathname)) {
    // Network first, cache only as the offline fallback: a cache-first script
    // would never pick up fixes, since the cache name only changes with this file.
    event.respondWith(
      fetch(event.request)
        .then((response) => {
          if (response.ok) {
            const copy = response.clone();
            caches.open(CACHE_NAME).then((cache) => cache.put(event.request, copy));
          }
          return response;
        })
        .catch(() => caches.match(event.request))
    );
  }
});
