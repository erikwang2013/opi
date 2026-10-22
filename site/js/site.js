/* opi static site — navigation, reveal, pet-form dialog. No framework. */
(() => {
  'use strict';

  const doc = document.documentElement;
  const scriptBase = document.currentScript?.src || location.href;
  doc.classList.add('js');

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Synchronous WebP probe; older browsers fall back to the original GIFs.
  const webpSupported = (() => {
    try {
      const probe = document.createElement('canvas');
      probe.width = 1;
      probe.height = 1;
      return probe.toDataURL('image/webp').startsWith('data:image/webp');
    } catch (_) {
      return false;
    }
  })();
  window.__opiWebp = webpSupported;
  if (!webpSupported) {
    $$('img[src$=".webp"]').forEach((img) => { img.src = img.src.replace(/\.webp$/, '.gif'); });
    $$('[data-anim$=".webp"]').forEach((el) => { el.dataset.anim = el.dataset.anim.replace(/\.webp$/, '.gif'); });
  }

  const year = $('#year');
  if (year) year.textContent = String(new Date().getFullYear());

  const header = $('.site-header');
  const progress = $('.scroll-progress span');
  const toTop = $('.back-to-top');
  const menuToggle = $('#menu-toggle');
  const siteNav = $('#site-nav');
  const sections = $$('main > section[id]');
  const navLinks = $$('.site-nav a[href^="#"]');
  const dots = $$('.section-dots a');
  let offsets = [];
  let activeId = '';
  let pageMax = 1;
  let viewportH = window.innerHeight;

  const measure = () => {
    offsets = sections.map((section) => ({
      id: section.id,
      top: section.offsetTop,
      height: section.offsetHeight,
    }));
    viewportH = window.innerHeight;
    pageMax = Math.max(1, doc.scrollHeight - viewportH);
  };

  const setActive = (id) => {
    if (!id || id === activeId) return;
    activeId = id;
    navLinks.forEach((link) => {
      if (link.hash === `#${id}`) link.setAttribute('aria-current', 'page');
      else link.removeAttribute('aria-current');
    });
    dots.forEach((dot) => {
      if (dot.dataset.for === id) dot.setAttribute('aria-current', 'true');
      else dot.removeAttribute('aria-current');
    });
  };

  let scheduled = false;
  const update = () => {
    scheduled = false;
    const y = window.scrollY;
    if (progress) progress.style.transform = `scaleX(${Math.min(1, y / pageMax)})`;
    if (header) header.classList.toggle('is-scrolled', y > 12);
    if (toTop) toTop.classList.toggle('is-visible', y > viewportH * 0.8);

    if (!offsets.length) measure();
    const probe = y + viewportH * 0.38;
    let current = offsets[0] && offsets[0].id;
    for (const item of offsets) {
      if (item.top <= probe) current = item.id;
    }
    setActive(current);
  };

  const requestUpdate = () => {
    if (scheduled) return;
    scheduled = true;
    window.requestAnimationFrame(update);
  };

  // Mobile menu: same nav in the header, shown below 960px.
  if (menuToggle && siteNav) {
    const setMenu = (open) => {
      siteNav.classList.toggle('is-open', open);
      menuToggle.setAttribute('aria-expanded', String(open));
      menuToggle.setAttribute('aria-label', open ? '关闭菜单' : '打开菜单');
    };
    menuToggle.addEventListener('click', () => {
      setMenu(!siteNav.classList.contains('is-open'));
    });
    siteNav.addEventListener('click', (event) => {
      if (event.target.closest('a')) setMenu(false);
    });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape' && siteNav.classList.contains('is-open')) {
        setMenu(false);
        menuToggle.focus();
      }
    });
    window.matchMedia('(min-width: 961px)').addEventListener('change', (event) => {
      if (event.matches) setMenu(false);
    });
  }

  toTop?.addEventListener('click', () => {
    window.scrollTo({ top: 0, behavior: reduceMotion ? 'auto' : 'smooth' });
  });

  window.addEventListener('scroll', requestUpdate, { passive: true });
  window.addEventListener('resize', () => { measure(); requestUpdate(); }, { passive: true });
  window.addEventListener('load', () => { measure(); requestUpdate(); }, { once: true });
  measure();
  update();

  // Reveal on entry — one observer for all sections and cards.
  const revealTargets = $$('.reveal');
  if ('IntersectionObserver' in window && !reduceMotion) {
    const revealObserver = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        entry.target.classList.add('is-visible');
        revealObserver.unobserve(entry.target);
      });
    }, { rootMargin: '0px 0px -8% 0px', threshold: 0.08 });
    revealTargets.forEach((el) => revealObserver.observe(el));
  } else {
    revealTargets.forEach((el) => el.classList.add('is-visible'));
  }

  // Reduced motion: use the static PNG in place of the GIF.
  if (reduceMotion) {
    $$('img[data-still]').forEach((img) => { img.src = img.dataset.still; });
  }

  // Keep the twelve form cards on static PNGs until their section is near; then play the WebP animation.
  const formCards = $$('.form-card');
  const setFormAnimation = (on) => {
    formCards.forEach((card) => {
      const img = card.querySelector('img');
      if (!img) return;
      const src = on ? card.dataset.anim : img.dataset.still;
      if (src && img.getAttribute('src') !== src) img.src = src;
    });
  };
  if (!reduceMotion) {
    setFormAnimation(false);
    const formsSection = $('#forms');
    if (formsSection && 'IntersectionObserver' in window) {
      const formObserver = new IntersectionObserver((entries) => {
        setFormAnimation(entries.some((entry) => entry.isIntersecting));
      }, { rootMargin: '300px 0px' });
      formObserver.observe(formsSection);
    } else {
      setFormAnimation(true);
    }
  }

  // Donation QR: enlarge in place instead of opening a new tab.
  const qrDialog = $('#qr-dialog');
  const qrImage = $('#qr-image');
  const qrTitle = $('#qr-title');
  let qrPreviousFocus = null;
  const closeQr = () => {
    if (!qrDialog || !qrDialog.hasAttribute('open')) return;
    if (typeof qrDialog.close === 'function') qrDialog.close();
    else qrDialog.removeAttribute('open');
    qrPreviousFocus?.focus?.({ preventScroll: true });
  };
  if (qrDialog) {
    $$('.qr-open').forEach((button) => {
      button.addEventListener('click', () => {
        const src = button.dataset.qrSrc;
        if (!src || !qrImage) return;
        qrPreviousFocus = button;
        qrImage.src = src;
        qrImage.alt = button.dataset.qrLabel || '收款二维码';
        if (qrTitle) qrTitle.textContent = button.dataset.qrLabel || '收款二维码';
        if (typeof qrDialog.showModal === 'function') qrDialog.showModal();
        else qrDialog.setAttribute('open', '');
        $('.qr-close', qrDialog)?.focus?.({ preventScroll: true });
      });
    });
    $('.qr-close', qrDialog)?.addEventListener('click', closeQr);
    qrDialog.addEventListener('click', (event) => {
      if (event.target === qrDialog) closeQr();
    });
    qrDialog.addEventListener('cancel', (event) => {
      event.preventDefault();
      closeQr();
    });
  }

  // Pet-form dialog. A non-modal dialog plus our own backdrop keeps opening well under 50 ms.
  const dialog = $('#form-dialog');
  const dialogBackdrop = $('#form-backdrop');
  const dialogImage = $('#dialog-image');
  const dialogName = $('#dialog-name');
  const dialogNote = $('#dialog-note');
  let dialogPreviousFocus = null;
  const closeDialog = () => {
    if (!dialog || !dialog.hasAttribute('open')) return;
    if (typeof dialog.close === 'function') dialog.close();
    else dialog.removeAttribute('open');
    if (dialogBackdrop) dialogBackdrop.hidden = true;
    document.body.classList.remove('form-modal-open');
    dialog.removeAttribute('aria-modal');
    dialogPreviousFocus?.focus?.({ preventScroll: true });
  };

  const openDialog = (card) => {
    if (dialog.hasAttribute('open')) return;
    const shown = card.querySelector('img');
    // Use the already-decoded card frame when possible: swapping in the
    // animation URL here forces a fresh decode and pushed the open past 50 ms.
    const loaded = shown?.currentSrc || shown?.getAttribute('src') || '';
    const anim = card.dataset.anim;
    const still = shown?.dataset.still;
    const next = loaded || (reduceMotion && still ? still : (anim || still || ''));
    if (dialogImage && next && dialogImage.getAttribute('src') !== next) dialogImage.src = next;
    if (dialogName) dialogName.textContent = card.dataset.name || '';
    if (dialogNote) dialogNote.textContent = card.dataset.note || '';
    dialogPreviousFocus = card;
    if (dialogBackdrop) dialogBackdrop.hidden = false;
    document.body.classList.add('form-modal-open');
    dialog.setAttribute('aria-modal', 'true');
    if (typeof dialog.show === 'function') dialog.show();
    else dialog.setAttribute('open', '');
    $('.dialog-close', dialog)?.focus?.({ preventScroll: true });
  };

  if (dialog) {
    $$('.form-card').forEach((card) => {
      card.addEventListener('click', () => openDialog(card));
    });
    $('.dialog-close', dialog)?.addEventListener('click', closeDialog);
    dialogBackdrop?.addEventListener('click', closeDialog);
    dialog.addEventListener('click', (event) => {
      if (event.target === dialog) closeDialog();
    });
    dialog.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        closeDialog();
        return;
      }
      if (event.key !== 'Tab') return;
      const focusable = $$('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])', dialog)
        .filter((el) => !el.disabled && el.offsetParent !== null);
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    });
  }

  // History scroller defaults to the LATEST release (top). Only materialise the
  // offscreen history when the board is near; never jump to the oldest entry.
  const releaseScroll = $('.release-scroll');
  if (releaseScroll) {
    const showLatest = () => {
      releaseScroll.style.contentVisibility = 'visible';
      window.requestAnimationFrame(() => { releaseScroll.scrollTop = 0; });
    };
    const releasesSection = $('#releases');
    if (location.hash === '#releases') {
      showLatest();
    } else if (releasesSection && 'IntersectionObserver' in window) {
      const releaseObserver = new IntersectionObserver((entries) => {
        if (!entries.some((entry) => entry.isIntersecting)) return;
        releaseObserver.disconnect();
        showLatest();
      }, { rootMargin: '500px 0px' });
      releaseObserver.observe(releasesSection);
    } else {
      showLatest();
    }
  }

  // Keep the active dot honest when arriving with a hash (e.g. /#download).
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target && target.id) setActive(target.id);
  }

  // Mark the primary download for the system in use; purely additive, no layout shift.
  const detectOS = () => {
    const ua = navigator.userAgent || '';
    const platform = String(navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
    if (/android/i.test(ua)) return 'android';
    if (/iphone|ipad|ipod/i.test(ua)) return 'ios';
    if (/win/i.test(platform)) return 'windows';
    if (/mac/i.test(platform)) return 'macos';
    if (/linux/i.test(platform)) return 'linux';
    return '';
  };
  const currentOS = detectOS();
  const currentCard = currentOS && document.querySelector(`.download-card.is-primary[data-os="${currentOS}"]`);
  if (currentCard) {
    currentCard.classList.add('is-current-os');
    const badge = document.createElement('span');
    badge.className = 'os-current';
    badge.textContent = '当前系统';
    currentCard.append(badge);
  }

  // three.js is decorative; load it after first paint, and never with reduced motion or Save-Data.
  const hasHardwareWebGL = () => {
    try {
      const probe = document.createElement('canvas');
      const gl = probe.getContext('webgl2') || probe.getContext('webgl');
      if (!gl) return false;
      const info = gl.getExtension('WEBGL_debug_renderer_info');
      const name = info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : '';
      gl.getExtension('WEBGL_lose_context')?.loseContext();
      return !/swiftshader|llvmpipe|software/i.test(name);
    } catch (_) {
      return false;
    }
  };
  const canUse3d = () => !reduceMotion && !navigator.connection?.saveData
    && !window.matchMedia('(max-width: 720px)').matches
    && (navigator.hardwareConcurrency || 8) >= 4
    && (navigator.deviceMemory || 8) > 2
    && hasHardwareWebGL();
  const startPet3d = () => {
    // Phones get the lightweight CSS pet; 3D stays on larger screens and always yields to interaction.
    const hero = $('#hero');
    if (!hero || !canUse3d()) return;
    const rect = hero.getBoundingClientRect();
    if (rect.bottom <= 0 || rect.top >= viewportH) return;
    import(new URL('pet3d.js', scriptBase).href).catch(() => { /* static pet stays */ });
  };
  if ('requestIdleCallback' in window) window.requestIdleCallback(startPet3d, { timeout: 1800 });
  else window.setTimeout(startPet3d, 900);

  // The 3D keycap loader fades as soon as the page is usable; never blocks content.
  const markReady = () => doc.classList.add('is-ready');
  if (document.readyState === 'complete') markReady();
  else window.addEventListener('load', markReady, { once: true });
  window.setTimeout(markReady, 2500);
})();
