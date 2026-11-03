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

  // PDF export. `tct` sends a card's PDF through the terminal stream as
  //   ESC ] 7733 ; <index> ; <total> ; <base64 file name> ; <base64 chunk> BEL
  // (see src/export/web.rs). Chunks arrive in order; index 0 starts a transfer.
  const PDF_OSC = 7733;
  const MAX_PDF_BYTES = 25 * 1024 * 1024;

  function decodeBase64(text) {
    return Uint8Array.from(atob(text), function (c) {
      return c.charCodeAt(0);
    });
  }

  // Keep only a plain file name: no paths, no control characters, ends in .pdf.
  function safePdfName(raw) {
    const base = String(raw).split(/[\\/]/).pop().replace(/[\u0000-\u001f\u007f<>:"|?*]/g, '').trim();
    const name = base.slice(0, 120) || 'card';
    return /\.pdf$/i.test(name) ? name : name + '.pdf';
  }

  // Collects chunks; `accept` returns { name, bytes } once the last one arrives
  // and the data is a PDF, otherwise null.
  function createPdfReceiver() {
    let chunks = [];
    let size = 0;
    let name = '';
    return {
      accept: function (data) {
        const parts = data.split(';');
        if (parts.length !== 4) {
          return null;
        }
        const index = Number(parts[0]);
        const total = Number(parts[1]);
        if (!Number.isInteger(index) || !Number.isInteger(total) || total < 1 || index >= total) {
          return null;
        }
        if (index === 0) {
          chunks = [];
          size = 0;
        }
        if (index !== chunks.length) {
          chunks = [];
          size = 0;
          return null;
        }
        let bytes;
        try {
          bytes = decodeBase64(parts[3]);
          name = safePdfName(new TextDecoder().decode(decodeBase64(parts[2])));
        } catch (_) {
          chunks = [];
          return null;
        }
        size += bytes.length;
        if (size > MAX_PDF_BYTES) {
          chunks = [];
          size = 0;
          return null;
        }
        chunks.push(bytes);
        if (chunks.length < total) {
          return null;
        }
        const all = new Uint8Array(size);
        let offset = 0;
        chunks.forEach(function (chunk) {
          all.set(chunk, offset);
          offset += chunk.length;
        });
        chunks = [];
        size = 0;
        const magic = String.fromCharCode.apply(null, Array.from(all.slice(0, 5)));
        return magic === '%PDF-' ? { name: name, bytes: all } : null;
      },
    };
  }

  // Hand the PDF to the user: the share sheet on touch devices (installed iOS
  // PWAs ignore the download attribute), a normal download elsewhere.
  function deliverPdf(pdf, term) {
    const blob = new Blob([pdf.bytes], { type: 'application/pdf' });
    function download() {
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = pdf.name;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      setTimeout(function () {
        URL.revokeObjectURL(url);
      }, 60000);
      if (term && typeof term.focus === 'function') {
        term.focus();
      }
    }
    const touch = window.matchMedia && window.matchMedia('(pointer: coarse)').matches;
    if (touch && typeof File === 'function' && navigator.canShare && navigator.share) {
      const file = new File([blob], pdf.name, { type: 'application/pdf' });
      if (navigator.canShare({ files: [file] })) {
        navigator.share({ files: [file] }).catch(download);
        return;
      }
    }
    download();
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

    // 4. Register the PDF export handler so `tct` can send a card as a download
    if (term.parser && typeof term.parser.registerOscHandler === 'function') {
      const receiver = createPdfReceiver();
      term.parser.registerOscHandler(PDF_OSC, function (data) {
        const pdf = receiver.accept(data);
        if (pdf) {
          deliverPdf(pdf, term);
        }
        return true;
      });
    }

    // 5. Re-fit whenever an addon (such as WebglAddon) is loaded onto the terminal
    if (typeof term.loadAddon === 'function') {
      const origLoadAddon = term.loadAddon.bind(term);
      term.loadAddon = function (addon) {
        const res = origLoadAddon(addon);
        scheduleFit();
        return res;
      };
    }
  }

  if (typeof module === 'object' && module.exports) {
    // Loaded by `node --test` (docker/pwa/tct-pwa.test.js): export the pure parts.
    module.exports = { createPdfReceiver: createPdfReceiver, safePdfName: safePdfName };
    return;
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
