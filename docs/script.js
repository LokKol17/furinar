/* ============================================================
   FURINAR — Landing Page Script
   Clean, lightweight, fast interactions.
   ============================================================ */

(function () {
  'use strict';

  // ============================================================
  // TRANSLATIONS (EN / PT-BR)
  // ============================================================
  const translations = {
    en: {
      'nav.features': 'Features',
      'nav.gallery': 'Gallery',
      'nav.download': 'Download',
      'nav.cta': 'Download',

      'hero.title': 'Your music.<br><em>Nothing else.</em>',
      'hero.subtitle': 'A native desktop music player that opens in milliseconds and stays out of your way — no browser engine, no unnecessary overhead.',
      'hero.ram': 'of RAM',
      'hero.tab': 'Less than a single browser tab',
      'hero.download_win': 'Download for Windows',
      'hero.view_github': 'View on GitHub',
      'hero.built_rust': 'Built with Rust',

      'light.label': 'Philosophy',
      'light.title': 'No browser engine.<br>No unnecessary overhead.',
      'light.body': 'Most music players ship an entire browser runtime to render their interface. Furinar is built natively with <strong>Rust and Slint</strong> — a compiled UI toolkit that produces a lean binary. The result: instant launch, minimal footprint, and a player that stays quietly in the background.',
      'light.p1': 'Opens in milliseconds',
      'light.p2': '2–10 MB of RAM — consistently',
      'light.p3': 'A single portable executable',
      'light.p4': 'No runtime dependencies on Windows',
      'light.card_browser': 'Browser tab',
      'light.card_note': 'Approximate RAM usage — your music stays yours.',

      'features.label': 'Features',
      'features.title': 'Everything a real player needs.',
      'features.sub': 'No cloud sync, no subscriptions, no tracking. Just your local library, played right.',

      'feat1.title': 'Full Playback Control',
      'feat1.desc': 'Play, pause, stop, seek, adjust volume, and navigate tracks with clean controls. Loop modes — off, single track, or full playlist — plus shuffle. Furinar picks up exactly where you left off on every reopen.',

      'feat2.title': 'Multiple Libraries as Tabs',
      'feat2.desc': 'Open as many folders as you want — each becomes its own tab. Switching between libraries never interrupts what\'s currently playing. Optional recursive subfolder scanning means no track is left behind.',
      'feat2.l1': 'Folder tabs — each library independent',
      'feat2.l2': 'Recursive subfolder scanning',
      'feat2.l3': 'ID3 / Vorbis tag reading — title, artist',
      'feat2.l4': 'Falls back gracefully to filename',

      'feat3.title': 'Instant Search & Navigation',
      'feat3.desc': 'Search by track title or artist across your active library. Type directly into the search field and results filter in real time with zero delay.',
      'feat3.l1': 'Live filtering as you type',
      'feat3.l2': 'Matches on title, artist, or filename',
      'feat3.l3': 'Preserves your active playlist state',
      'search.placeholder': 'buscar por título ou artista...',

      'feat4.title': 'Synchronized Lyrics (LRC)',
      'feat4.desc': 'Furinar automatically detects and loads synced .lrc files alongside your tracks. The lyrics view highlights lines in sync with playback time.',
      'feat4.l1': 'Auto-loads .lrc files with same basename',
      'feat4.l2': 'Real-time line synchronization',
      'feat4.l3': 'Clean scrolling lyrics view',

      'feat5.title': 'Native System Integration',
      'feat5.desc': 'Furinar feels like part of the OS. On Windows, playback controls appear directly in the taskbar thumbnail and in the system media overlay. On Linux, MPRIS integration connects it to desktop media controls.',
      'feat5.l1': 'Windows taskbar thumbnail controls',
      'feat5.l2': 'SMTC — system media keys & action center',
      'feat5.l3': 'MPRIS on Linux',
      'feat5.kbd_prefix': 'Keyboard:',

      'kbd.play_pause': 'Play / Pause',
      'kbd.seek': 'Seek ± 5s',
      'kbd.nav': 'Navigate list',
      'kbd.play_sel': 'Play selected',

      'showcase.label': 'Gallery',
      'showcase.title': 'See it in action.',
      'showcase.cap1': 'Dark Theme — Playlist & Controls',
      'showcase.cap2': 'Light Theme',
      'showcase.cap3': 'Synchronized Lyrics (.lrc)',

      'native.label': 'Native',
      'native.title': 'A desktop app.<br>Actually a desktop app.',
      'native.body': 'Furinar is a compiled binary — not an Electron wrapper, not a web view. It integrates cleanly with the OS: frameless window, custom title bar, Aero Snap, rounded corners on Windows 11, and native media controls on both platforms. Configuration is a plain JSON file you can edit by hand.',
      'native.d1': 'Built with',
      'native.d2': 'Config',
      'native.v2': 'JSON file, editable by hand',
      'native.d3': 'Theme',
      'native.v3': 'Light & Dark — switch instantly',
      'native.d4': 'Updates',
      'native.v4': 'Built-in auto-update mechanism',

      'download.label': 'Download',
      'download.title': 'Ready to listen?',
      'download.sub': 'Free, open source, and ready to run. No installation wizard, no account.',
      'download.win_note': 'x86-64 · Single executable',
      'download.linux_note': 'x86-64 · Tarball with install script',
      'download.btn_win': 'Download .exe',
      'download.btn_linux': 'Download .tar.gz',
      'download.other': 'Other install methods for Linux',
      'install.universal': 'Universal installer',

      'links.releases': 'All releases',
      'links.github': 'GitHub repository',
      'links.docs': 'Documentation',
      'links.license': 'License',

      'copy.btn': 'Copy',
      'copy.done': 'Copied!',

      'footer.tagline': 'A light, fast, and beautiful audio player.',
      'footer.back_to_top': 'Back to top ↑',
      'footer.copy': 'BSD-3-Clause · Open source · Fan project inspired by Furina from <em>Genshin Impact</em> (HoYoverse) · No official affiliation'
    },
    pt: {
      'nav.features': 'Recursos',
      'nav.gallery': 'Galeria',
      'nav.download': 'Download',
      'nav.cta': 'Baixar',

      'hero.title': 'Sua música.<br><em>Nada além.</em>',
      'hero.subtitle': 'Um player de música nativo para desktop que abre em milissegundos e não atrapalha — sem engine de navegador, sem peso desnecessário.',
      'hero.ram': 'de RAM',
      'hero.tab': 'Menos que uma única aba do navegador',
      'hero.download_win': 'Baixar para Windows',
      'hero.view_github': 'Ver no GitHub',
      'hero.built_rust': 'Feito com Rust',

      'light.label': 'Filosofia',
      'light.title': 'Sem engine de navegador.<br>Sem peso desnecessário.',
      'light.body': 'A maioria dos players carrega um navegador inteiro embutido para renderizar a interface. O Furinar é construído nativamente com <strong>Rust e Slint</strong> — gerando um binário leve e veloz. Resultado: abertura instantânea, consumo mínimo de recursos e uma reprodução silenciosa em segundo plano.',
      'light.p1': 'Abre em milissegundos',
      'light.p2': '2–10 MB de RAM — estável',
      'light.p3': 'Executável único e portátil',
      'light.p4': 'Sem dependências no Windows',
      'light.card_browser': 'Aba do navegador',
      'light.card_note': 'Uso aproximado de RAM — foco total na sua música.',

      'features.label': 'Recursos',
      'features.title': 'Tudo o que um player precisa.',
      'features.sub': 'Sem sincronização na nuvem, sem assinaturas, sem rastreamento. Apenas sua biblioteca local, tocada com perfeição.',

      'feat1.title': 'Controle Total de Reprodução',
      'feat1.desc': 'Play, pause, stop, busca rápida, ajuste de volume e navegação fluida. Modos de repetição — desativado, faixa única ou playlist completa — além de shuffle. O Furinar retoma de onde você parou sempre que for reaberto.',

      'feat2.title': 'Múltiplas Pastas em Abas',
      'feat2.desc': 'Abra quantas pastas quiser — cada uma vira sua própria aba independente. Alternar entre bibliotecas nunca interrompe a música atual. Varredura recursiva de subpastas para não perder nenhuma faixa.',
      'feat2.l1': 'Abas de pastas — cada biblioteca independente',
      'feat2.l2': 'Varredura recursiva de subpastas opcional',
      'feat2.l3': 'Leitura de tags ID3 / Vorbis (título, artista)',
      'feat2.l4': 'Fallback inteligente para o nome do arquivo',

      'feat3.title': 'Busca Instantânea & Navegação',
      'feat3.desc': 'Busque por título ou artista na sua pasta ativa. Digite no campo de busca e os resultados são filtrados em tempo real com zero atraso.',
      'feat3.l1': 'Filtragem ao vivo enquanto você digita',
      'feat3.l2': 'Busca por título, artista ou nome de arquivo',
      'feat3.l3': 'Preserva o estado da sua playlist ativa',
      'search.placeholder': 'buscar por título ou artista...',

      'feat4.title': 'Letras Sincronizadas (LRC)',
      'feat4.desc': 'O Furinar detecta e carrega automaticamente arquivos .lrc com o mesmo nome da música. A visualização de letras destaca as frases sincronizadas com o tempo da faixa.',
      'feat4.l1': 'Carregamento automático de arquivos .lrc',
      'feat4.l2': 'Sincronização em tempo real das linhas',
      'feat4.l3': 'Visualização de letras com rolagem fluida',

      'feat5.title': 'Integração Nativa com o Sistema',
      'feat5.desc': 'O Furinar se integra ao sistema operacional. No Windows, os controles de reprodução aparecem na miniatura da barra de tarefas e no overlay de mídia (SMTC). No Linux, a integração MPRIS conecta aos controles de mídia do desktop.',
      'feat5.l1': 'Controles na miniatura da barra de tarefas (Windows)',
      'feat5.l2': 'SMTC — teclas multimídia e central do sistema',
      'feat5.l3': 'MPRIS no Linux',
      'feat5.kbd_prefix': 'Atalhos:',

      'kbd.play_pause': 'Play / Pause',
      'kbd.seek': 'Avançar / Retroceder 5s',
      'kbd.nav': 'Navegar na lista',
      'kbd.play_sel': 'Tocar selecionada',

      'showcase.label': 'Galeria',
      'showcase.title': 'Veja em ação.',
      'showcase.cap1': 'Tema Escuro — Playlist & Controles',
      'showcase.cap2': 'Tema Claro',
      'showcase.cap3': 'Letras Sincronizadas (.lrc)',

      'native.label': 'Nativo',
      'native.title': 'Um app desktop.<br>De verdade.',
      'native.body': 'O Furinar é um binário nativo compilado — não é Electron nem visualizador web. Integração total com o sistema: janela frameless, barra de título personalizada, Aero Snap, cantos arredondados no Windows 11 e controles de mídia nativos. Configurações em arquivo JSON simples.',
      'native.d1': 'Construído com',
      'native.d2': 'Configuração',
      'native.v2': 'Arquivo JSON editável à mão',
      'native.d3': 'Tema',
      'native.v3': 'Claro e Escuro — troca instantânea',
      'native.d4': 'Atualizações',
      'native.v4': 'Mecanismo de auto-update embutido',

      'download.label': 'Download',
      'download.title': 'Pronto para ouvir?',
      'download.sub': 'Gratuito, open source e pronto para rodar. Sem instalador pesado, sem conta.',
      'download.win_note': 'x86-64 · Executável único portátil',
      'download.linux_note': 'x86-64 · Pacote com script de instalação',
      'download.btn_win': 'Baixar .exe',
      'download.btn_linux': 'Baixar .tar.gz',
      'download.other': 'Outros métodos de instalação para Linux',
      'install.universal': 'Instalador universal',

      'links.releases': 'Todos os lançamentos',
      'links.github': 'Repositório GitHub',
      'links.docs': 'Documentação',
      'links.license': 'Licença',

      'copy.btn': 'Copiar',
      'copy.done': 'Copiado!',

      'footer.tagline': 'Um player de áudio leve, rápido e simples.',
      'footer.back_to_top': 'Voltar ao topo ↑',
      'footer.copy': 'BSD-3-Clause · Open source · Projeto de fã inspirado em Furina de <em>Genshin Impact</em> (HoYoverse) · Sem afiliação oficial'
    }
  };

  let currentLang = 'en';

  // Try to load saved language preference or default to browser language
  try {
    const saved = localStorage.getItem('furinar_lang');
    if (saved && (saved === 'en' || saved === 'pt')) {
      currentLang = saved;
    } else if (navigator.language && navigator.language.toLowerCase().startsWith('pt')) {
      currentLang = 'pt';
    }
  } catch (e) {}

  function applyLanguage(lang) {
    currentLang = lang;
    try {
      localStorage.setItem('furinar_lang', lang);
    } catch (e) {}

    document.documentElement.lang = lang;

    // Update all i18n data elements
    const dict = translations[lang] || translations.en;
    document.querySelectorAll('[data-i18n]').forEach((el) => {
      const key = el.getAttribute('data-i18n');
      if (dict[key] !== undefined) {
        el.innerHTML = dict[key];
      }
    });

    // Update switcher buttons active state
    document.querySelectorAll('.lang-opt').forEach((opt) => {
      const isEn = opt.classList.contains('lang-en');
      const isPt = opt.classList.contains('lang-pt');
      if (lang === 'en') {
        opt.classList.toggle('active', isEn);
      } else {
        opt.classList.toggle('active', isPt);
      }
    });
  }

  // Toggle button listeners
  const langBtn = document.getElementById('lang-btn');
  const mobileLangBtn = document.getElementById('mobile-lang-btn');

  function toggleLanguage() {
    applyLanguage(currentLang === 'en' ? 'pt' : 'en');
  }

  if (langBtn) langBtn.addEventListener('click', toggleLanguage);
  if (mobileLangBtn) mobileLangBtn.addEventListener('click', toggleLanguage);

  // Apply initial language
  applyLanguage(currentLang);

  // ============================================================
  // COPY TO CLIPBOARD BUTTONS
  // ============================================================
  document.querySelectorAll('.copy-btn').forEach((btn) => {
    btn.addEventListener('click', async () => {
      const textToCopy = btn.getAttribute('data-copy');
      if (!textToCopy) return;

      try {
        await navigator.clipboard.writeText(textToCopy);
        const textSpan = btn.querySelector('.copy-text');
        const originalText = textSpan ? textSpan.textContent : '';
        const copiedLabel = translations[currentLang]?.['copy.done'] || 'Copied!';

        btn.classList.add('copied');
        if (textSpan) textSpan.textContent = copiedLabel;

        setTimeout(() => {
          btn.classList.remove('copied');
          if (textSpan) {
            const defaultLabel = translations[currentLang]?.['copy.btn'] || 'Copy';
            textSpan.textContent = defaultLabel;
          }
        }, 2000);
      } catch (err) {
        console.warn('Could not copy to clipboard:', err);
      }
    });
  });

  // ============================================================
  // NAVBAR: scroll-triggered frosted glass
  // ============================================================
  const navbar = document.querySelector('.navbar');
  if (navbar) {
    const onScroll = () => {
      navbar.classList.toggle('scrolled', window.scrollY > 16);
    };
    window.addEventListener('scroll', onScroll, { passive: true });
    onScroll();
  }

  // ============================================================
  // MOBILE MENU
  // ============================================================
  const burger = document.querySelector('.nav-burger');
  const mobileMenu = document.getElementById('mobile-menu');
  if (burger && mobileMenu) {
    burger.addEventListener('click', () => {
      const isOpen = mobileMenu.classList.toggle('open');
      burger.setAttribute('aria-expanded', String(isOpen));
      mobileMenu.setAttribute('aria-hidden', String(!isOpen));
    });

    mobileMenu.querySelectorAll('a').forEach((link) => {
      link.addEventListener('click', () => {
        mobileMenu.classList.remove('open');
        burger.setAttribute('aria-expanded', 'false');
        mobileMenu.setAttribute('aria-hidden', 'true');
      });
    });

    document.addEventListener('click', (e) => {
      if (!navbar.contains(e.target)) {
        mobileMenu.classList.remove('open');
        burger.setAttribute('aria-expanded', 'false');
        mobileMenu.setAttribute('aria-hidden', 'true');
      }
    });
  }

  // ============================================================
  // REVEAL ON SCROLL
  // ============================================================
  const revealElements = document.querySelectorAll('.reveal');
  if (revealElements.length > 0 && 'IntersectionObserver' in window) {
    const revealObserver = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add('visible');
            revealObserver.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.12, rootMargin: '0px 0px -40px 0px' }
    );
    revealElements.forEach((el) => revealObserver.observe(el));
  } else {
    revealElements.forEach((el) => el.classList.add('visible'));
  }

  // ============================================================
  // SMOOTH SCROLL for anchor links
  // ============================================================
  document.querySelectorAll('a[href^="#"]').forEach((link) => {
    link.addEventListener('click', function (e) {
      const id = this.getAttribute('href');
      if (id === '#') return;
      const target = document.querySelector(id);
      if (target) {
        e.preventDefault();
        const navH = navbar ? navbar.offsetHeight : 0;
        const top = target.getBoundingClientRect().top + window.scrollY - navH - 8;
        window.scrollTo({ top, behavior: 'smooth' });
      }
    });
  });

  // ============================================================
  // HERO SCREENSHOT: subtle parallax on mousemove
  // ============================================================
  const heroWrap = document.querySelector('.hero-screenshot-wrap');
  if (heroWrap && window.matchMedia('(pointer: fine)').matches) {
    let raf = null;
    let tx = 0, ty = 0, cx = 0, cy = 0;

    document.addEventListener('mousemove', (e) => {
      const mx = (e.clientX / window.innerWidth - 0.5) * 2;
      const my = (e.clientY / window.innerHeight - 0.5) * 2;
      tx = mx * 4;
      ty = my * 2;
    });

    const animate = () => {
      cx += (tx - cx) * 0.06;
      cy += (ty - cy) * 0.06;
      heroWrap.style.transform = `translate(${cx.toFixed(2)}px, ${cy.toFixed(2)}px)`;
      raf = requestAnimationFrame(animate);
    };
    animate();

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        cancelAnimationFrame(raf);
      } else {
        animate();
      }
    });
  }

  // ============================================================
  // PLAYBACK DEMO ANIMATION (Feature 01)
  // ============================================================
  (function () {
    const progress  = document.getElementById('demo-progress');
    const thumb     = document.getElementById('demo-thumb');
    const timeCur   = document.getElementById('demo-time-cur');
    const timeDur   = document.getElementById('demo-time-dur');
    const trackName = document.getElementById('demo-track-name');
    const playBtn   = document.getElementById('demo-play-btn');
    if (!progress || !thumb || !timeCur) return;

    const TRACKS = [
      { name: 'my!lane \u2014 This Feeling',           dur: 201 },
      { name: 'Eve \u2014 \u591c\u306e\u697d\u30b8\u30fc\u30af\u30ec\u30c3\u30c8',               dur: 187 },
      { name: 'PinocchioP \u2014 Isn\u2019t it \u201cA\u201d',        dur: 163 },
      { name: 'FORGOTTENAGE \u2014 Valhalla',          dur: 218 },
    ];

    const PLAY_SVG  = '<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>';
    const PAUSE_SVG = '<svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>';

    const fmt = (s) => {
      const m = Math.floor(s / 60).toString().padStart(2, '0');
      const sec = (s % 60).toString().padStart(2, '0');
      return `${m}:${sec}`;
    };

    let trackIdx = 0;
    let elapsed  = 48; // start mid-song
    let playing  = true;
    let raf      = null;
    let lastTs   = null;

    const loadTrack = (idx, startAt = 0) => {
      const t = TRACKS[idx];
      elapsed = startAt;
      timeDur.textContent = fmt(t.dur);
      if (trackName) {
        trackName.style.opacity = '0';
        setTimeout(() => {
          trackName.textContent = t.name;
          trackName.style.opacity = '1';
        }, 200);
      }
    };

    loadTrack(trackIdx, elapsed);

    const tick = (ts) => {
      if (!playing) { lastTs = null; raf = requestAnimationFrame(tick); return; }
      if (lastTs !== null) {
        elapsed += (ts - lastTs) / 1000;
      }
      lastTs = ts;

      const dur = TRACKS[trackIdx].dur;
      if (elapsed >= dur) {
        // Pause briefly then skip to next track
        playing = false;
        if (playBtn) playBtn.innerHTML = PLAY_SVG;
        setTimeout(() => {
          trackIdx = (trackIdx + 1) % TRACKS.length;
          loadTrack(trackIdx, 0);
          playing = true;
          if (playBtn) playBtn.innerHTML = PAUSE_SVG;
        }, 900);
      }

      const pct = Math.min((elapsed / dur) * 100, 100);
      progress.style.width = `${pct}%`;
      thumb.style.left     = `${pct}%`;
      timeCur.textContent  = fmt(Math.floor(elapsed));

      raf = requestAnimationFrame(tick);
    };

    raf = requestAnimationFrame(tick);

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) { lastTs = null; }
    });
  })();

  // ============================================================
  // TYPEWRITER SEARCH ANIMATION (Feature 03)
  // ============================================================
  (function () {
    const typed   = document.getElementById('search-typed');
    const results = document.getElementById('search-results');
    if (!typed || !results) return;

    // Each scenario: query to type + matching results
    const SCENARIOS = [
      {
        query: 'pinocchio',
        items: [
          { text: 'PinocchioP \u2014 Isn\u2019t it \u201cA\u201d',  active: true  },
          { text: 'PinocchioP \u2014 \u8150\u308c\u5916\u9053\u3068\u30c1\u30e7\u30b3\u30ec\u3090\u30c8', active: false },
        ],
      },
      {
        query: '\u591c\u306e\u697d',
        items: [
          { text: 'Eve \u2014 \u591c\u306e\u697d\u30b8\u30fc\u30af\u30ec\u30c3\u30c8', active: true },
        ],
      },
      {
        query: 'valhalla',
        items: [
          { text: 'FORGOTTENAGE \u2014 Valhalla', active: true },
        ],
      },
      {
        query: 'my!lane',
        items: [
          { text: 'my!lane \u2014 This Feeling',  active: true  },
          { text: 'my!lane \u2014 Interlude',     active: false },
        ],
      },
      {
        query: 'blxnk',
        items: [
          { text: 'blxnk \u2014 por vc',           active: true  },
          { text: 'blxnk \u2014 tarde d+',          active: false },
          { text: 'blxnk \u2014 coisas pra falar',  active: false },
          { text: 'blxnk \u2014 sempre por voc\u00ea', active: false },
          { text: 'blxnk \u2014 v\u00edcio',           active: false },
        ],
      },
    ];

    const CHAR_DELAY  = 90;   // ms per character typed
    const DELETE_DELAY = 50;  // ms per character deleted
    const HOLD        = 1600; // ms to hold full query before deleting
    const CLEAR_PAUSE = 500;  // ms pause after clearing before next query

    let scenarioIdx = 0;
    let charIdx     = 0;
    let phase       = 'type'; // 'type' | 'hold' | 'delete' | 'pause'
    let timer       = null;

    const renderResults = (query) => {
      const sc = SCENARIOS[scenarioIdx];
      // Only show results once query matches scenario
      if (query.length < 2) { results.innerHTML = ''; return; }
      results.innerHTML = sc.items.map(it =>
        `<div class="search-result-item${it.active ? ' search-result-item--active' : ''}">${it.text}</div>`
      ).join('');
    };

    const step = () => {
      const sc = SCENARIOS[scenarioIdx];
      const full = sc.query;

      if (phase === 'type') {
        charIdx++;
        const current = full.slice(0, charIdx);
        typed.textContent = current;
        renderResults(current);
        if (charIdx >= full.length) {
          phase = 'hold';
          timer = setTimeout(step, HOLD);
        } else {
          timer = setTimeout(step, CHAR_DELAY);
        }

      } else if (phase === 'hold') {
        phase = 'delete';
        timer = setTimeout(step, DELETE_DELAY);

      } else if (phase === 'delete') {
        charIdx--;
        typed.textContent = full.slice(0, charIdx);
        renderResults(full.slice(0, charIdx));
        if (charIdx <= 0) {
          results.innerHTML = '';
          phase = 'pause';
          timer = setTimeout(step, CLEAR_PAUSE);
        } else {
          timer = setTimeout(step, DELETE_DELAY);
        }

      } else if (phase === 'pause') {
        scenarioIdx = (scenarioIdx + 1) % SCENARIOS.length;
        charIdx = 0;
        phase   = 'type';
        timer   = setTimeout(step, CHAR_DELAY);
      }
    };

    // Start after a short initial delay
    timer = setTimeout(step, 800);

    document.addEventListener('visibilitychange', () => {
      if (document.hidden) {
        clearTimeout(timer);
      } else {
        timer = setTimeout(step, CHAR_DELAY);
      }
    });
  })();

  // ============================================================
  // SYNCHRONIZED LYRICS ANIMATION (Feature 04) - Seamless infinite loop
  // ============================================================
  (function () {
    const track = document.getElementById('lyrics-track');
    if (!track) return;

    const origLines = Array.from(track.querySelectorAll('.lyrics-line'));
    const N = origLines.length;
    if (N === 0) return;

    // Build: [clone-before] [original] [clone-after]
    // Prepend clones in reverse so order is preserved
    for (let i = N - 1; i >= 0; i--) {
      track.insertBefore(origLines[i].cloneNode(true), track.firstChild);
    }
    origLines.forEach(line => track.appendChild(line.cloneNode(true)));

    const allLines = Array.from(track.querySelectorAll('.lyrics-line'));
    // total = 3*N lines. Middle set is [N .. 2N-1].
    // Start at line N+2 so we're visibly mid-song
    let idx = N + 2;

    const centerOffset = 70; // px — half viewport height

    const getY = (i) => {
      const el = allLines[i];
      if (!el) return 0;
      return el.offsetTop - centerOffset + el.offsetHeight / 2;
    };

    const applyHighlights = (i) => {
      allLines.forEach((line, k) => {
        line.classList.remove('lyrics-line--active', 'lyrics-line--past');
        if (k === i) line.classList.add('lyrics-line--active');
        else if (k === i - 1 || k === i - 2) line.classList.add('lyrics-line--past');
      });
    };

    // Place without animation
    track.style.transition = 'none';
    track.style.transform = `translateY(-${Math.max(0, getY(idx))}px)`;
    applyHighlights(idx);
    void track.offsetHeight; // flush

    // After each smooth scroll completes, silently reset position if
    // we've drifted into the third (cloned) set
    let resetting = false;
    track.addEventListener('transitionend', (e) => {
      if (e.propertyName !== 'transform' || resetting) return;
      if (idx >= N * 2) {
        resetting = true;
        idx -= N;

        // Freeze ALL per-line CSS transitions so that changing classes
        // during the reset doesn't trigger a visible fade/scale
        allLines.forEach(l => { l.style.transition = 'none'; });

        track.style.transition = 'none';
        track.style.transform = `translateY(-${Math.max(0, getY(idx))}px)`;
        applyHighlights(idx);

        // Single flush: commits position + class changes in one frame
        void track.offsetHeight;

        // Restore per-line transitions for next normal step
        allLines.forEach(l => { l.style.transition = ''; });

        resetting = false;
      }
    });

    // Advance one line
    const advance = () => {
      idx++;
      applyHighlights(idx);
      track.style.transition = 'transform 0.45s cubic-bezier(0.25, 0.8, 0.25, 1)';
      track.style.transform = `translateY(-${Math.max(0, getY(idx))}px)`;
    };

    let timer = null;
    const start = () => { if (!timer) timer = setInterval(advance, 1800); };
    const stop  = () => { clearInterval(timer); timer = null; };

    start();
    document.addEventListener('visibilitychange', () =>
      document.hidden ? stop() : start()
    );
  })();

  // ============================================================
  // BACK TO TOP
  // ============================================================
  const backToTopBtn = document.getElementById('back-to-top');
  if (backToTopBtn) {
    backToTopBtn.addEventListener('click', (e) => {
      e.preventDefault();
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }

})();
