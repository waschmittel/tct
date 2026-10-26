const CACHE_NAME = 'tct-pwa-v1';
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
    event.respondWith(
      caches.match(event.request).then((cached) => cached || fetch(event.request))
    );
  }
});
