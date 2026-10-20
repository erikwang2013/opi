/* opi static site — memory timeline overlay. Loaded with defer after site.js. */
(() => {
  'use strict';
  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  const memoryOverlay = $('#memory-overlay');
  const memoryViewport = $('#memory-viewport');
  const memoryTimeline = $('#memory-timeline');
  const memoryProgress = $('#memory-progress');
  const memoryIntro = $('#memory-intro');
  const memoryClose = memoryOverlay ? $('[data-memory-close]', memoryOverlay) : null;
  const memorySeenKey = 'opi-memory-seen-v1';
  const memoryPets = [
    'idle.gif', 'waiting.gif', 'crying.gif', 'puzzled.gif', 'terrified.gif',
    'angry.gif', 'coquettish.gif', 'cute.gif', 'pleasing.gif', 'knocking.gif',
    'face-palm.gif', 'tongue-out.gif',
  ];
  const memoryIntroForms = [
    ['idle.gif', '小欧·笑', '#F5A6B4'], ['waiting.gif', '小欧·欢乐', '#F27D9D'],
    ['crying.gif', '小欧·哭', '#4E97E6'], ['puzzled.gif', '小欧·沮丧', '#7E8C86'],
    ['terrified.gif', '小欧·惊恐', '#6FB2F0'], ['angry.gif', '小欧·愤怒', '#DD5138'],
    ['coquettish.gif', '小欧·撒娇', '#F46F9B'], ['cute.gif', '小欧·卖萌', '#F2B84B'],
    ['pleasing.gif', '小欧·讨好', '#5FBF9F'], ['knocking.gif', '小欧·敲打', '#E8A13C'],
    ['face-palm.gif', '小欧·捂脸', '#E06A4E'], ['tongue-out.gif', '小欧·吐舌头', '#EF5D8F'],
  ];
  const preloadMemoryPets = () => {
    memoryIntroForms.forEach(([file]) => { const img = new Image(); img.src = `assets/pet/forms/${file}`; });
  };

  let memoryBuilt = false;
  let memoryIntroAll = null;
  let memoryIntroGrid = null;
  let memoryIntroAllCount = null;
  let memoryRaf = 0;
  let memoryEndTimer = 0;
  let memoryIntroTimer = 0;
  let memoryLoadTimer = 0;
  let memoryObserver = null;
  let memoryRevealTimer = 0;
  let memoryRevealQueue = [];
  let memoryLastReveal = 0;
  let memoryPreviousFocus = null;
  let memoryPaused = false;
  const MEMORY_LOADING_MIN_MS = 2800;
  const MEMORY_LOADING_MAX_MS = 4200;

  const markMemorySeen = () => {
    try { localStorage.setItem(memorySeenKey, '1'); } catch (_) { /* private mode */ }
  };

  const buildMemory = () => {
    if (memoryBuilt || !memoryTimeline) return;
    memoryBuilt = true;
    const source = $$('.release-latest, .release-item a', $('.release-board') || document);
    const rows = source.map((item) => ({
      version: $('.release-version', item)?.textContent?.trim() || '',
      date: $('.release-date', item)?.textContent?.trim() || '',
      note: $('.release-note', item)?.textContent?.trim() || '',
    })).filter((row) => row.version).reverse(); // oldest → newest (top → bottom)
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

  const buildMemoryIntro = () => {
    if (!memoryIntro || memoryIntro.children.length) return;
    const all = document.createElement('div');
    all.className = 'memory-intro-pane memory-intro-all';
    const title = document.createElement('p');
    title.className = 'memory-intro-title';
    title.textContent = '小欧的十二种形态';
    const grid = document.createElement('div');
    grid.className = 'memory-intro-grid';
    const allCount = document.createElement('span');
    all.append(title, grid, allCount);
    memoryIntroForms.forEach(([file, name, mood]) => {
      const item = document.createElement('figure');
      item.className = 'memory-intro-item';
      item.style.setProperty('--mood', mood);
      const petImg = document.createElement('img');
      petImg.src = `assets/pet/forms/${file}`;
      petImg.width = 64;
      petImg.height = 64;
      petImg.alt = name;
      petImg.loading = 'eager';
      petImg.decoding = 'async';
      const caption = document.createElement('figcaption');
      caption.textContent = name;
      item.append(petImg, caption);
      grid.append(item);
    });
    memoryIntro.append(all);
    memoryIntroAll = all;
    memoryIntroGrid = grid;
    memoryIntroAllCount = allCount;
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
    if (memoryIntroTimer) clearTimeout(memoryIntroTimer);
    if (memoryLoadTimer) clearTimeout(memoryLoadTimer);
    if (memoryRevealTimer) clearTimeout(memoryRevealTimer);
    memoryObserver?.disconnect();
    memoryObserver = null;
    memoryRevealQueue = [];
    memoryRaf = 0;
    memoryEndTimer = 0;
    memoryIntroTimer = 0;
    memoryLoadTimer = 0;
    memoryRevealTimer = 0;
    memoryIntro?.querySelectorAll('img').forEach((img) => img.getAnimations().forEach((a) => a.cancel()));
    memoryIntro?.classList.remove('is-showing');
    memoryIntroAll?.classList.remove('is-active');
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

  const resetMemoryItems = () => {
    if (!memoryTimeline) return;
    $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => {
      el.classList.remove('is-in', 'is-current');
      delete el.dataset.memoryQueued;
    });
  };

  const revealMemoryNext = () => {
    memoryRevealTimer = 0;
    if (memoryPaused) {
      memoryRevealTimer = window.setTimeout(revealMemoryNext, 250);
      return;
    }
    const next = memoryRevealQueue.shift();
    if (next) {
      if (next.classList.contains('memory-item')) {
        const current = memoryTimeline?.querySelector('.memory-item.is-current');
        if (current && current !== next) current.classList.remove('is-current');
        next.classList.add('is-in', 'is-current');
      } else {
        next.classList.add('is-in');
      }
      delete next.dataset.memoryQueued;
      memoryLastReveal = performance.now();
    }
    if (memoryRevealQueue.length) {
      const wait = Math.max(0, 1100 - (performance.now() - memoryLastReveal));
      memoryRevealTimer = window.setTimeout(revealMemoryNext, wait);
    }
  };

  const queueMemoryReveal = (el) => {
    if (!el || el.classList.contains('is-in') || el.dataset.memoryQueued) return;
    el.dataset.memoryQueued = '1';
    const wait = Math.max(0, 1100 - (performance.now() - memoryLastReveal));
    memoryRevealQueue.push(el);
    if (!memoryRevealTimer) {
      if (wait === 0) revealMemoryNext();
      else memoryRevealTimer = window.setTimeout(revealMemoryNext, wait);
    }
  };

  const introEntry = (index) => {
    const entry = index % 3;
    if (entry === 0) return 'translate3d(-70vw, 18vh, 0) scale(.18) rotate(-150deg)';
    if (entry === 1) return 'translate3d(70vw, 30vh, 0) scale(.2) rotate(140deg)';
    return 'translate3d(0, 62vh, 0) scale(.16) rotate(80deg)';
  };

  const startMemoryIntro = () => {
    if (!memoryIntro || reduceMotion) { playMemory(); return; }
    buildMemoryIntro();
    stopMemory();
    const started = performance.now();
    const items = $$('.memory-intro-item', memoryIntroGrid);
    const images = items.map((item) => $('img', item));
    let loaded = 0;
    let finished = false;
    memoryIntro.classList.add('is-showing');
    memoryIntroAll.classList.add('is-active');

    const finish = () => {
      if (finished) return;
      finished = true;
      if (memoryIntroTimer) clearTimeout(memoryIntroTimer);
      if (memoryLoadTimer) clearTimeout(memoryLoadTimer);
      memoryIntro.classList.remove('is-showing');
      playMemory();
    };
    const checkLoaded = () => {
      if (finished) return;
      const elapsed = performance.now() - started;
      if ((elapsed >= MEMORY_LOADING_MIN_MS && loaded >= images.length) ||
          elapsed >= MEMORY_LOADING_MAX_MS) {
        finish();
        return;
      }
      memoryLoadTimer = window.setTimeout(checkLoaded, 120);
    };
    images.forEach((img) => {
      const done = () => { loaded += 1; };
      if (img.complete) done();
      else { img.addEventListener('load', done, { once: true }); img.addEventListener('error', done, { once: true }); }
    });
    checkLoaded();

    items.forEach((item) => { item.style.opacity = '0'; item.classList.remove('is-current'); });
    let index = 0;
    const enter = () => {
      if (finished) return;
      const item = items[index];
      memoryIntro.style.setProperty('--intro-mood', memoryIntroForms[index][2]);
      item.style.opacity = '1';
      item.animate(
        [{ transform: introEntry(index) }, { transform: 'scale(1.08) rotate(-2deg)', offset: .78 }, { transform: 'none' }],
        { duration: 560, easing: 'cubic-bezier(.16,1,.3,1)' },
      );
      index += 1;
      memoryIntroAllCount.textContent = `加载中 · ${index} / ${memoryIntroForms.length}`;
      if (index < items.length) memoryIntroTimer = window.setTimeout(enter, 130);
      else cycleMoods();
    };
    const cycleMoods = () => {
      if (finished) return;
      let active = 0;
      const next = () => {
        if (finished) return;
        items.forEach((item, i) => item.classList.toggle('is-current', i === active));
        memoryIntro.style.setProperty('--intro-mood', memoryIntroForms[active][2]);
        active = (active + 1) % items.length;
        memoryIntroTimer = window.setTimeout(next, 420);
      };
      next();
    };
    enter();
  };

  const playMemory = () => {
    if (!memoryTimeline || !memoryViewport || !memoryProgress) return;
    stopMemory();
    resetMemoryItems();
    memoryViewport.scrollTop = 0;
    const firstItem = memoryTimeline.querySelector('.memory-item');
    if (firstItem) firstItem.classList.add('is-in', 'is-current');
    memoryLastReveal = performance.now();
    memoryProgressNow();
    if (reduceMotion) {
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => el.classList.add('is-in'));
      return;
    }
    if ('IntersectionObserver' in window) {
      memoryObserver = new IntersectionObserver((entries) => {
        entries.filter((entry) => entry.isIntersecting)
          .sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)
          .forEach((entry) => queueMemoryReveal(entry.target));
      }, { root: memoryViewport, rootMargin: '0px 0px -10% 0px', threshold: 0.08 });
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => memoryObserver.observe(el));
    } else {
      $$('.memory-item, .memory-pet', memoryTimeline).forEach((el) => el.classList.add('is-in'));
    }
    const max = Math.max(0, memoryViewport.scrollHeight - memoryViewport.clientHeight);
    if (max <= 0) return;
    const duration = 120000;
    const speed = max / duration;
    let last = 0;
    const step = (now) => {
      if (!last) last = now;
      const dt = Math.min(80, Math.max(0, now - last));
      last = now;
      if (!memoryPaused) {
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
    memoryOverlay.hidden = false;
    document.body.classList.add('memory-open');
    markMemorySeen();
    memoryClose?.focus?.({ preventScroll: true });
    window.requestAnimationFrame(() => {
      memoryOverlay.classList.add('is-open');
      startMemoryIntro();
    });
  };

  if (memoryOverlay) {
    if ('requestIdleCallback' in window) window.requestIdleCallback(preloadMemoryPets, { timeout: 1800 });
    else window.setTimeout(preloadMemoryPets, 600);
    $$('[data-memory-open]').forEach((button) => button.addEventListener('click', openMemory));
    memoryClose?.addEventListener('click', closeMemory);
    memoryOverlay.addEventListener('click', (event) => { if (event.target === memoryOverlay) closeMemory(); });
    memoryViewport?.addEventListener('pointerenter', () => setMemoryPaused(true));
    memoryViewport?.addEventListener('pointerleave', () => setMemoryPaused(false));
    memoryViewport?.addEventListener('scroll', memoryProgressNow, { passive: true });
    document.addEventListener('keydown', (event) => { if (event.key === 'Escape') closeMemory(); });
    let seen = false;
    try { seen = localStorage.getItem(memorySeenKey) === '1'; } catch (_) { /* private mode */ }
    if (!seen) window.setTimeout(openMemory, 1200);
  }
})();
