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
      'hero.ram': "of RAM while playing",
      'hero.tab': 'Less than a single browser tab',
      'hero.download_win': 'Download for Windows',
      'hero.view_github': 'View on GitHub',
      'hero.built_rust': 'Built with Rust',

      'light.label': 'Philosophy',
      'light.title': 'No browser engine.<br>No unnecessary overhead.',
      'light.body': 'Most music players ship an entire browser runtime to render their interface. Furinar is built natively with <strong>Rust and Slint</strong> — a compiled UI toolkit that produces a lean binary. The result: instant launch, minimal footprint, and a player that stays quietly in the background.',
      'light.p1': 'Opens in milliseconds',
      'light.p2': "~12 MB idle, ~20 MB playing",
      'light.p3': 'A single portable executable',
      'light.p4': 'No runtime dependencies on Windows',
      'light.card_browser': 'Browser tab',
      'light.card_note': 'Approximate RAM usage — your music stays yours.',

      'features.label': 'Features',
      'features.title': 'Everything a real player needs.',
      'features.sub': 'No cloud sync, no subscriptions, no tracking. Just your local library, played right.',

      'feat1.title': "Playback That Gets the Details Right",
      'feat1.desc': "Always gapless, with optional crossfade that leaves consecutive tracks of the same album alone, and ReplayGain so every track plays at the same loudness. Smart shuffle plays every track once before repeating, a sleep timer fades you out, and Furinar reopens paused right where you left off.",

      'feat2.title': "Your Library, Organized",
      'feat2.desc': "Open as many folders as you want — each becomes its own tab, and switching never interrupts what's playing. Favorites and Most played gather tracks from every folder, and each tab remembers how you like it sorted.",
      'feat2.l1': "Folder tabs, plus Favorites and Most played",
      'feat2.l2': "Sort by title, artist, album, duration, date added or plays",
      'feat2.l3': "Tags: title, artist, album, track number, duration",
      'feat2.l4': "Optional subfolder scanning",

      'feat3.title': 'Instant Search & Navigation',
      'feat3.desc': 'Search by track title or artist across your active library. Type directly into the search field and results filter in real time with zero delay.',
      'feat3.l1': 'Live filtering as you type',
      'feat3.l2': 'Matches on title, artist, or filename',
      'feat3.l3': 'Preserves your active playlist state',
      'search.placeholder': 'buscar por título ou artista...',

      'feat4.title': "Synced Lyrics — Even Karaoke",
      'feat4.desc': ".lrc files next to your tracks, or lyrics embedded in the tags. Missing ones can be fetched from LRCLIB. In fullscreen, the current line fills as it's sung — word by word with enhanced LRC.",
      'feat4.l1': ".lrc files and embedded lyrics (SYLT / USLT)",
      'feat4.l2': "Optional download from LRCLIB",
      'feat4.l3': "Karaoke-style fill in fullscreen",

      'feat5.title': 'Native System Integration',
      'feat5.desc': "Furinar feels like part of the OS. On Windows, playback controls appear in the taskbar thumbnail and the system media overlay; on Linux, MPRIS connects it to your desktop's media controls. It can also show what you're listening to on Discord and scrobble to ListenBrainz or Last.fm.",
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
      'showcase.cap4': 'Immersive Mode (F11)',
      'showcase.cap5': 'Library Drawer (L)',
      'showcase.cap6': 'Lyrics Only (H)',
      'showcase.cap7': 'Tags & Lyrics from the Web (I)',
      'showcase.cap8': 'Mini Player (M)',

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
      'download.sub': 'Free, open source, and ready to run. A quick installer, no admin rights, no account.',
      'download.win_note': 'x86-64 · Installer, no admin needed',
      'download.win_portable': 'Or the portable .exe',
      'download.linux_note': 'x86-64 · Tarball with install script',
      'download.btn_win': 'Download installer',
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
      'footer.copy': 'BSD-3-Clause · Open source · Fan project inspired by Furina from <em>Genshin Impact</em> (HoYoverse) · No official affiliation',
      'nav.news': "What's new",
      'news.label': "What's new · 1.7",
      'news.title': "The biggest update yet.<br>Still light.",
      'news.sub': "A fullscreen mode made for listening, smarter playback, and your library tidied up from the web — while staying around 20 MB of RAM. Everything heavy only exists while you use it.",
      'news.c1t': "Immersive mode",
      'news.c1d': "Fullscreen with the cover's accent color, animated or video backgrounds, a spectrum visualizer, karaoke lyrics and an album grid.",
      'news.c2t': "Playback, refined",
      'news.c2d': "No silence between tracks, optional crossfade, ReplayGain loudness, smart shuffle and a sleep timer.",
      'news.c3t': "Favorites & sorting",
      'news.c3d': "Favorites and Most played tabs across all folders, and each tab remembers its own sort order.",
      'news.c4t': "Metadata & lyrics from the web",
      'news.c4d': "Identify tracks on MusicBrainz — even by their sound — fetch covers and synced lyrics, and fix a whole tab at once.",
      'news.c5t': "Discord & scrobbling",
      'news.c5d': "\"Listening to…\" on your Discord profile, and every play scrobbled to ListenBrainz or Last.fm.",
      'news.c6t': "Mini player",
      'news.c6d': "The window shrinks into a small always-on-top bar with the cover and playback controls.",
      'news.more': "Full release notes →",
      'feat5.l4': "Discord status, ListenBrainz & Last.fm",
      'kbd.fav': "Favorite",
      'kbd.mini': "Mini player",
      'kbd.imv': "Immersive mode",
      'feat6.title': "Immersive Fullscreen Mode",
      'feat6.desc': "Press F11 and the music takes the whole screen: the cover, an accent color taken from it, your own backgrounds — images, animated GIF/WebP or video — and a spectrum visualizer. A library drawer brings the album grid and the up-next queue. Everything it uses is freed when you leave.",
      'feat6.l1': "Backgrounds that rotate on every track",
      'feat6.l2': "Spectrum visualizer and karaoke lyrics",
      'feat6.l3': "Library drawer, album grid and up-next queue",
      'feat6.l4': "Lyrics-only view, clock and sleep timer",
      'imv.l1': "The room goes dark, the music stays",
      'imv.l2': "Every word lights up as it's sung",
      'imv.l3': "line by line, until the end",
      'imv.title': "Your favorite song",
      'imv.artist': "Demo · Furinar 1.7",
      'feat7.title': "Fix Your Tags From the Web",
      'feat7.desc': "Messy tags or files called \"track01.mp3\"? Press I: Furinar looks the track up on MusicBrainz — or by its sound, with AcoustID — and shows candidates with a confidence score. Index a whole tab at once; fixes stay in Furinar until you choose to write them to your files.",
      'feat7.l1': "MusicBrainz search and AcoustID fingerprinting",
      'feat7.l2': "Covers from the Cover Art Archive",
      'feat7.l3': "Whole-tab indexing with review",
      'feat7.l4': "Nothing touches your files without confirmation",
      'web.search': "Search the web",
      'web.by_sound': "Cover Band · matched by sound"
    },
    pt: {
      'nav.features': 'Recursos',
      'nav.gallery': 'Galeria',
      'nav.download': 'Download',
      'nav.cta': 'Baixar',

      'hero.title': 'Sua música.<br><em>Nada além.</em>',
      'hero.subtitle': 'Um player de música nativo para desktop que abre em milissegundos e não atrapalha — sem engine de navegador, sem peso desnecessário.',
      'hero.ram': "de RAM tocando",
      'hero.tab': 'Menos que uma única aba do navegador',
      'hero.download_win': 'Baixar para Windows',
      'hero.view_github': 'Ver no GitHub',
      'hero.built_rust': 'Feito com Rust',

      'light.label': 'Filosofia',
      'light.title': 'Sem engine de navegador.<br>Sem peso desnecessário.',
      'light.body': 'A maioria dos players carrega um navegador inteiro embutido para renderizar a interface. O Furinar é construído nativamente com <strong>Rust e Slint</strong> — gerando um binário leve e veloz. Resultado: abertura instantânea, consumo mínimo de recursos e uma reprodução silenciosa em segundo plano.',
      'light.p1': 'Abre em milissegundos',
      'light.p2': "~12 MB parado, ~20 MB tocando",
      'light.p3': 'Executável único e portátil',
      'light.p4': 'Sem dependências no Windows',
      'light.card_browser': 'Aba do navegador',
      'light.card_note': 'Uso aproximado de RAM — foco total na sua música.',

      'features.label': 'Recursos',
      'features.title': 'Tudo o que um player precisa.',
      'features.sub': 'Sem sincronização na nuvem, sem assinaturas, sem rastreamento. Apenas sua biblioteca local, tocada com perfeição.',

      'feat1.title': "Reprodução Caprichada nos Detalhes",
      'feat1.desc': "Sempre sem silêncio entre faixas, com crossfade opcional que não mexe em faixas seguidas do mesmo álbum, e ReplayGain para todas tocarem no mesmo volume. O shuffle inteligente toca cada faixa uma vez antes de repetir, o temporizador de sono baixa o volume aos poucos, e o Furinar reabre pausado onde você parou.",

      'feat2.title': "Sua Biblioteca, Organizada",
      'feat2.desc': "Abra quantas pastas quiser — cada uma vira uma aba, e trocar de aba nunca interrompe o que está tocando. Favoritas e Mais tocadas juntam faixas de todas as pastas, e cada aba lembra a ordem que você escolheu.",
      'feat2.l1': "Abas de pastas, mais Favoritas e Mais tocadas",
      'feat2.l2': "Ordenar por título, artista, álbum, duração, data de adição ou reproduções",
      'feat2.l3': "Tags: título, artista, álbum, número da faixa, duração",
      'feat2.l4': "Varredura de subpastas opcional",

      'feat3.title': 'Busca Instantânea & Navegação',
      'feat3.desc': 'Busque por título ou artista na sua pasta ativa. Digite no campo de busca e os resultados são filtrados em tempo real com zero atraso.',
      'feat3.l1': 'Filtragem ao vivo enquanto você digita',
      'feat3.l2': 'Busca por título, artista ou nome de arquivo',
      'feat3.l3': 'Preserva o estado da sua playlist ativa',
      'search.placeholder': 'buscar por título ou artista...',

      'feat4.title': "Letras Sincronizadas — Até Karaokê",
      'feat4.desc': "Arquivos .lrc ao lado das faixas ou letras embutidas nas tags. As que faltam podem ser baixadas do LRCLIB. Na tela cheia, a linha atual enche no ritmo do canto — palavra por palavra com LRC estendido.",
      'feat4.l1': "Arquivos .lrc e letras embutidas (SYLT / USLT)",
      'feat4.l2': "Download opcional do LRCLIB",
      'feat4.l3': "Preenchimento estilo karaokê na tela cheia",

      'feat5.title': 'Integração Nativa com o Sistema',
      'feat5.desc': "O Furinar se integra ao sistema. No Windows, os controles aparecem na miniatura da barra de tarefas e no overlay de mídia; no Linux, o MPRIS conecta aos controles de mídia do desktop. Ele também pode mostrar no Discord o que você está ouvindo e fazer scrobble no ListenBrainz ou no Last.fm.",
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
      'showcase.cap4': 'Modo Imersivo (F11)',
      'showcase.cap5': 'Painel da Biblioteca (L)',
      'showcase.cap6': 'Só Letra (H)',
      'showcase.cap7': 'Tags e Letras da Web (I)',
      'showcase.cap8': 'Mini Player (M)',

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
      'download.sub': 'Gratuito, open source e pronto para rodar. Instalador rápido, sem precisar de administrador, sem conta.',
      'download.win_note': 'x86-64 · Instalador, sem precisar de administrador',
      'download.win_portable': 'Ou o .exe portátil',
      'download.linux_note': 'x86-64 · Pacote com script de instalação',
      'download.btn_win': 'Baixar instalador',
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
      'footer.copy': 'BSD-3-Clause · Open source · Projeto de fã inspirado em Furina de <em>Genshin Impact</em> (HoYoverse) · Sem afiliação oficial',
      'nav.news': "Novidades",
      'news.label': "Novidades · 1.7",
      'news.title': "A maior atualização até agora.<br>Ainda leve.",
      'news.sub': "Um modo tela cheia feito para ouvir, uma reprodução mais caprichada e a sua biblioteca arrumada pela web — usando cerca de 20 MB de RAM. O que é pesado só existe enquanto você usa.",
      'news.c1t': "Modo imersivo",
      'news.c1d': "Tela cheia com a cor da capa, fundos animados ou em vídeo, visualizador de espectro, letra estilo karaokê e grade de álbuns.",
      'news.c2t': "Reprodução caprichada",
      'news.c2d': "Sem silêncio entre faixas, crossfade opcional, volume igualado pelo ReplayGain, shuffle inteligente e temporizador de sono.",
      'news.c3t': "Favoritas e ordenação",
      'news.c3d': "Abas de Favoritas e Mais tocadas juntando todas as pastas, e cada aba lembra a própria ordem.",
      'news.c4t': "Metadados e letras da web",
      'news.c4d': "Identifique faixas no MusicBrainz — até pelo som —, baixe capas e letras sincronizadas e corrija uma aba inteira de uma vez.",
      'news.c5t': "Discord e scrobble",
      'news.c5d': "\"Ouvindo…\" no seu perfil do Discord, e cada faixa registrada no ListenBrainz ou no Last.fm.",
      'news.c6t': "Mini player",
      'news.c6d': "A janela vira uma barrinha sempre por cima, com a capa e os controles.",
      'news.more': "Notas completas da versão →",
      'feat5.l4': "Status no Discord, ListenBrainz e Last.fm",
      'kbd.fav': "Favoritar",
      'kbd.mini': "Mini player",
      'kbd.imv': "Modo imersivo",
      'feat6.title': "Modo Imersivo em Tela Cheia",
      'feat6.desc': "Aperte F11 e a música toma a tela toda: a capa, uma cor de destaque tirada dela, seus próprios fundos — imagens, GIF/WebP animado ou vídeo — e um visualizador de espectro. Um painel lateral traz a grade de álbuns e a fila \"a seguir\". Tudo que ele usa é liberado quando você sai.",
      'feat6.l1': "Fundos que trocam a cada faixa",
      'feat6.l2': "Visualizador de espectro e letra karaokê",
      'feat6.l3': "Painel da biblioteca, grade de álbuns e fila \"a seguir\"",
      'feat6.l4': "Modo só letra, relógio e temporizador de sono",
      'imv.l1': "A sala apaga, a música fica",
      'imv.l2': "Cada palavra acende quando é cantada",
      'imv.l3': "linha por linha, até o fim",
      'imv.title': "Sua música favorita",
      'imv.artist': "Demo · Furinar 1.7",
      'feat7.title': "Arrume suas Tags pela Web",
      'feat7.desc': "Tags bagunçadas ou arquivos chamados \"faixa01.mp3\"? Aperte I: o Furinar procura a faixa no MusicBrainz — ou pelo som, com o AcoustID — e mostra os candidatos com nota de confiança. Indexe uma aba inteira de uma vez; as correções ficam no Furinar até você decidir gravar nos arquivos.",
      'feat7.l1': "Busca no MusicBrainz e impressão digital do AcoustID",
      'feat7.l2': "Capas do Cover Art Archive",
      'feat7.l3': "Indexação da aba inteira, com revisão",
      'feat7.l4': "Nada mexe nos seus arquivos sem confirmação",
      'web.search': "Buscar na web",
      'web.by_sound': "Cover Band · identificada pelo som"
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
