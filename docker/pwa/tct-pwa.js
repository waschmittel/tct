// tct PWA & xterm.js integration hooks for ttyd.
(function () {
  'use strict';

  // Copy text to the browser clipboard, falling back to a hidden <textarea>
  // when navigator.clipboard is unavailable or rejected.
  function copyTextToClipboard(text, term) {
    function fallbackCopy() {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      try {
        document.execCommand('copy');
      } catch (_) {
        // Ignore copy failure
      }
      document.body.removeChild(textarea);
      if (term && typeof term.focus === 'function') {
        term.focus();
      }
    }

    if (navigator.clipboard && typeof navigator.clipboard.writeText === 'function') {
      navigator.clipboard.writeText(text).catch(fallbackCopy);
    } else {
      fallbackCopy();
    }
  }

  // Handle OSC 52 clipboard escape sequences (`ESC ] 52 ; c ; <base64> BEL`).
  function handleOsc52(data, term) {
    const sep = data.indexOf(';');
    if (sep < 0) {
      return false;
    }
    try {
      const base64 = data.slice(sep + 1);
      const bytes = Uint8Array.from(atob(base64), function (c) {
        return c.charCodeAt(0);
      });
      const text = new TextDecoder().decode(bytes);
      copyTextToClipboard(text, term);
    } catch (_) {
      // Ignore malformed base64 payload
    }
    return true;
  }

  function fitTerminal() {
    if (window.term && typeof window.term.fit === 'function') {
      window.term.fit();
    }
  }

  function scheduleFit() {
    requestAnimationFrame(fitTerminal);
    setTimeout(fitTerminal, 50);
    setTimeout(fitTerminal, 200);
  }

  function configureTerminal(term) {
    // 1. Disable scrollback so FitAddon does not reserve a 15px scrollbar gutter
    term.options.scrollback = 0;

    // 2. Ensure true black background before the terminal opens
    term.options.theme = Object.assign({}, term.options.theme, {
      background: '#000000',
      black: '#000000',
    });

    // 3. Register OSC 52 handler so `tct` can copy to the browser clipboard
    if (term.parser && typeof term.parser.registerOscHandler === 'function') {
      term.parser.registerOscHandler(52, function (data) {
        return handleOsc52(data, term);
      });
    }

    // 4. Re-fit whenever an addon (such as WebglAddon) is loaded onto the terminal
    if (typeof term.loadAddon === 'function') {
      const origLoadAddon = term.loadAddon.bind(term);
      term.loadAddon = function (addon) {
        const res = origLoadAddon(addon);
        scheduleFit();
        return res;
      };
    }
  }

  // Intercept `window.term = t` in ttyd's `open()` method so we can configure
  // the xterm.js instance before `t.open()` and the initial `fit()` run.
  let currentTerm;
  Object.defineProperty(window, 'term', {
    configurable: true,
    enumerable: true,
    get: function () {
      return currentTerm;
    },
    set: function (term) {
      currentTerm = term;
      if (term) {
        configureTerminal(term);
      }
    },
  });

  window.addEventListener('load', function () {
    if ('serviceWorker' in navigator) {
      navigator.serviceWorker.register('/sw.js');
    }
    scheduleFit();
    if (document.fonts && document.fonts.ready) {
      document.fonts.ready.then(fitTerminal);
    }
  });
})();
