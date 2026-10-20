/* opi static site — navigation, reveal, pet-form dialog. No framework. */
(() => {
  'use strict';

  const doc = document.documentElement;
  doc.classList.add('js');

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const year = $('#year');
  if (year) year.textContent = String(new Date().getFullYear());

  const header = $('.site-header');
  const progress = $('.scroll-progress span');
  const toTop = $('.back-to-top');
  const sections = $$('main > section[id]');
  const navLinks = $$('.site-nav a[href^="#"]');
  const dots = $$('.section-dots a');
  let offsets = [];
  let activeId = '';

  const measure = () => {
    offsets = sections.map((section) => ({
      id: section.id,
      top: section.offsetTop,
      height: section.offsetHeight,
    }));
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
    const max = Math.max(1, doc.scrollHeight - window.innerHeight);
    if (progress) progress.style.transform = `scaleX(${Math.min(1, y / max)})`;
    if (header) header.classList.toggle('is-scrolled', y > 12);
    if (toTop) toTop.classList.toggle('is-visible', y > window.innerHeight * 0.8);

    if (!offsets.length) measure();
    const probe = y + window.innerHeight * 0.38;
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

  // Pet-form dialog.
  const dialog = $('#form-dialog');
  const dialogImage = $('#dialog-image');
  const dialogName = $('#dialog-name');
  const dialogNote = $('#dialog-note');

  const closeDialog = () => {
    if (dialog && dialog.open) dialog.close();
  };

  if (dialog && typeof dialog.showModal === 'function') {
    $$('.form-card').forEach((card) => {
      card.addEventListener('click', () => {
        const gif = card.dataset.gif;
        const still = card.querySelector('img')?.dataset.still;
        if (dialogImage) dialogImage.src = reduceMotion && still ? still : (gif || still || '');
        if (dialogName) dialogName.textContent = card.dataset.name || '';
        if (dialogNote) dialogNote.textContent = card.dataset.note || '';
        dialog.showModal();
      });
    });

    $('.dialog-close', dialog)?.addEventListener('click', closeDialog);
    dialog.addEventListener('click', (event) => {
      if (event.target === dialog) closeDialog();
    });
  }

  // History memory timeline: first visit auto-plays once; the "回忆" button replays it.
  const memoryOverlay = $('#memory-overlay');
  const memoryViewport = $('#memory-viewport');
  const memoryTimeline = $('#memory-timeline');
  const memoryProgress = $('#memory-progress');
  const memoryClose = memoryOverlay ? $('[data-memory-close]', memoryOverlay) : null;
  const memorySeenKey = 'opi-memory-seen-v1';
  const memoryPets = [
    'idle.gif', 'waiting.png', 'crying.gif', 'puzzled.png', 'terrified.png',
    'angry.png', 'coquettish.gif', 'cute.gif', 'pleasing.png', 'knocking.gif',
    'face-palm.gif', 'tongue-out.gif',
  ];
  let memoryBuilt = false;
  let memoryRaf = 0;
  let memoryEndTimer = 0;
  let memoryAutoHoldUntil = 0;
  let memoryPreviousFocus = null;
  let memoryPaused = false;

  const markMemorySeen = () => {
    try { localStorage.setItem(memorySeenKey, '1'); } catch (_) { /* private mode */ }
  };

  const buildMemory = () => {
    if (memoryBuilt || !memoryTimeline) return;
    memoryBuilt = true;
    const source = $$('.release-latest, .release-item a', $('.release-board') || document);
    const rows = source.map((link) => {
      const item = link.classList.contains('release-latest') ? link : link;
      return {
        version: $('.release-version', item)?.textContent?.trim() || '',
        date: $('.release-date', item)?.textContent?.trim() || '',
        note: $('.release-note', item)?.textContent?.trim() || '',
      };
    }).filter((row) => row.version).reverse(); // oldest → newest
    const nodes = [];
    rows.forEach((row, index) => {
      const li = document.createElement('li');
      li.className = 'memory-item' + (index === rows.length - 1 ? ' is-latest' : '');
      const version = document.createElement('span');
      version.className = 'memory-version';
      version.textContent = row.version;
      const date = document.createElement('span');
      date.className = 'memory-date';
      date.textContent = row.date;
      const note = document.createElement('span');
      note.className = 'memory-note';
      note.textContent = row.note;
      li.append(version, date, note);
      nodes.push(li);
      if (index % 6 === 2) {
        const pet = document.createElement('li');
        pet.className = 'memory-pet' + (index % 12 === 9 ? ' is-right' : '');
        pet.setAttribute('aria-hidden', 'true');
        const img = document.createElement('img');
        img.src = `assets/pet/forms/${memoryPets[(index / 6 | 0) % memoryPets.length]}`;
        img.width = 54;
        img.height = 54;
        img.loading = 'lazy';
        img.decoding = 'async';
        img.alt = '';
        pet.append(img);
        nodes.push(pet);
      }
    });
    memoryTimeline.replaceChildren(...nodes);
  };

  const setMemoryPaused = (paused) => { memoryPaused = paused; };

  const memoryProgressNow = () => {
    if (!memoryViewport || !memoryProgress) return;
    const max = Math.max(1, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    memoryProgress.style.transform = `scaleX(${Math.min(1, memoryViewport.scrollTop / max)})`;
  };

  const stopMemory = () => {
    if (memoryRaf) cancelAnimationFrame(memoryRaf);
    if (memoryEndTimer) clearTimeout(memoryEndTimer);
    memoryRaf = 0;
    memoryEndTimer = 0;
  };

  const closeMemory = () => {
    if (!memoryOverlay || !memoryOverlay.classList.contains('is-open')) return;
    stopMemory();
    memoryOverlay.classList.remove('is-open');
    document.body.classList.remove('memory-open');
    memoryPreviousFocus?.focus?.({ preventScroll: true });
    window.setTimeout(() => {
      if (memoryOverlay && !memoryOverlay.classList.contains('is-open')) memoryOverlay.hidden = true;
    }, 240);
  };

  const playMemory = () => {
    if (!memoryTimeline || !memoryViewport || !memoryProgress) return;
    stopMemory();
    memoryViewport.scrollTop = 0;
    memoryProgressNow();
    if (reduceMotion) return;
    const max = Math.max(0, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    if (max <= 0) return;
    // ~2 minutes for 76 versions; the wheel scrolls natively and pauses auto-advance.
    const speed = max / 120000; // px per ms
    let last = 0;
    const step = (now) => {
      if (!last) last = now;
      const dt = Math.min(80, Math.max(0, now - last));
      last = now;
      if (!memoryPaused && now >= memoryAutoHoldUntil) {
        memoryViewport.scrollTop = Math.min(max, memoryViewport.scrollTop + speed * dt);
        if (memoryViewport.scrollTop >= max - 1) {
          memoryProgressNow();
          memoryEndTimer = window.setTimeout(closeMemory, 1200);
          return;
        }
      }
      memoryProgressNow();
      memoryRaf = requestAnimationFrame(step);
    };
    memoryRaf = requestAnimationFrame(step);
  };

  const openMemory = () => {
    if (!memoryOverlay) return;
    buildMemory();
    if (memoryOverlay.classList.contains('is-open')) return;
    memoryPreviousFocus = document.activeElement;
    memoryPaused = false;
    memoryAutoHoldUntil = 0;
    memoryViewport.scrollTop = 0;
    memoryOverlay.hidden = false;
    document.body.classList.add('memory-open');
    markMemorySeen();
    memoryClose?.focus?.({ preventScroll: true });
    window.requestAnimationFrame(() => {
      memoryOverlay.classList.add('is-open');
      playMemory();
    });
  };

  if (memoryOverlay) {
    $$('[data-memory-open]').forEach((button) => button.addEventListener('click', openMemory));
    memoryClose?.addEventListener('click', closeMemory);
    memoryOverlay.addEventListener('click', (event) => {
      if (event.target === memoryOverlay) closeMemory();
    });
    memoryViewport?.addEventListener('pointermove', () => { if (!memoryPaused) setMemoryPaused(true); });
    memoryViewport?.addEventListener('pointerleave', () => setMemoryPaused(false));
    memoryViewport?.addEventListener('click', () => setMemoryPaused(!memoryPaused));
    memoryViewport?.addEventListener('wheel', () => {
      memoryAutoHoldUntil = performance.now() + 8000;
    }, { passive: true });
    memoryViewport?.addEventListener('scroll', memoryProgressNow, { passive: true });
    document.addEventListener('keydown', (event) => {
      if (event.key === 'Escape') closeMemory();
    });
    let seen = false;
    try { seen = localStorage.getItem(memorySeenKey) === '1'; } catch (_) { /* private mode */ }
    if (!seen) window.setTimeout(openMemory, 1200);
  }

  // Keep the active dot honest when arriving with a hash (e.g. /#download).
  if (location.hash) {
    const target = document.querySelector(location.hash);
    if (target && target.id) setActive(target.id);
  }
})();
