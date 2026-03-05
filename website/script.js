/* ================================================
   ARTCODE Website — script.js
   Mobile nav, scroll animations, docs navigation
   Interactive Editor, Canvas BG Layer
   ================================================ */

// — Mobile Nav Toggle ————————————————————
const navToggle = document.getElementById('navToggle');
const navLinks = document.getElementById('navLinks');
if (navToggle && navLinks) {
    navToggle.addEventListener('click', () => {
        navLinks.classList.toggle('open');
        navToggle.textContent = navLinks.classList.contains('open') ? '✕' : '☰';
    });
}

// — Scroll Fade-in Animation —————————————
const observerOptions = { threshold: 0.1, rootMargin: '0px 0px -40px 0px' };
const fadeObserver = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
        if (entry.isIntersecting) {
            entry.target.classList.add('visible');
            fadeObserver.unobserve(entry.target);
        }
    });
}, observerOptions);

document.querySelectorAll('.fade-in').forEach(el => fadeObserver.observe(el));

// — Docs Page: Sidebar Navigation ————————
const docsSidebar = document.getElementById('docsSidebar');
if (docsSidebar) {
    const sidebarLinks = docsSidebar.querySelectorAll('a[data-doc]');
    const articles = document.querySelectorAll('.doc-article');

    // Show first article by default, hide others
    articles.forEach((article, i) => {
        if (i === 0) {
            article.style.display = 'block';
            article.classList.add('active');
        } else {
            article.style.display = 'none';
        }
    });

    sidebarLinks.forEach(link => {
        link.addEventListener('click', (e) => {
            e.preventDefault();
            const targetId = link.getAttribute('data-doc');
            // href can be #id or data-doc
            const hash = link.getAttribute('href')?.replace('#', '') || targetId;

            // Hide all articles, show target
            articles.forEach(a => {
                a.style.display = 'none';
                a.classList.remove('active');
            });

            const target = document.getElementById(hash);
            if (target) {
                target.style.display = 'block';
                target.classList.add('active');
                // Smooth scroll to top of content
                target.scrollIntoView({ behavior: 'smooth', block: 'start' });
            }

            // Update active link
            sidebarLinks.forEach(l => l.classList.remove('active'));
            link.classList.add('active');

            // Update URL hash without jumping
            history.replaceState(null, '', '#' + hash);
        });
    });

    // Handle initial hash on page load
    const hash = window.location.hash.replace('#', '');
    if (hash) {
        const targetLink = docsSidebar.querySelector(`a[href="#${hash}"]`);
        if (targetLink) targetLink.click();
    }
}

// — Nav background on scroll ——————————————
const nav = document.getElementById('nav');
if (nav) {
    window.addEventListener('scroll', () => {
        if (window.scrollY > 20) {
            nav.style.background = 'rgba(0, 0, 0, 0.95)';
        } else {
            nav.style.background = 'rgba(0, 0, 0, 0.8)';
        }
    }, { passive: true });
}

// — Interactive Typist for Hero ——————
const editorCode = document.getElementById('hero-editor-code');
const editorWrapper = document.getElementById('hero-editor-wrapper');
if (editorCode) {
    const phases = [
        {
            text: `<span class="cm">// Nível 1: Scripts simples e limpos</span>\n<span class="kw">let</span> <span class="var">nome</span> <span class="op">=</span> <span class="str">"Artcode"</span>\n<span class="fn">println</span>(<span class="str">f"Olá, </span><span class="op">{</span><span class="var">nome</span><span class="op">}</span><span class="str">! 🚀"</span>)`,
            delayAfter: 3500
        },
        {
            text: `<span class="cm">// Nível 2: Arrays, Enums e Pattern Matching</span>\n<span class="kw">enum</span> <span class="type">Status</span> { <span class="type">Ok</span>, <span class="type">Err</span>(<span class="type">String</span>) }\n\n<span class="kw">func</span> <span class="fn">check</span>() <span class="op">-></span> <span class="type">Status</span> {\n    <span class="kw">let</span> <span class="var">v</span> <span class="op">=</span> [<span class="num">1</span>, <span class="num">2</span>, <span class="num">3</span>]\n    <span class="kw">return</span> <span class="op">.</span><span class="type">Err</span>(<span class="str">"Falha na rede"</span>)\n}`,
            delayAfter: 4000
        },
        {
            text: `<span class="cm">// Nível 3: Performant blocks e Arenas (bypass do GC)</span>\n<span class="kw">performant</span> {\n    <span class="kw">arena</span> (<span class="num">1</span> <span class="op">*</span> <span class="num">1024</span> <span class="op">*</span> <span class="num">1024</span>) <span class="kw">as</span> <span class="var">frame</span> {\n        <span class="kw">let</span> <span class="var">p</span> <span class="op">=</span> <span class="var">frame</span><span class="op">.</span><span class="fn">alloc</span>(<span class="type">Entity</span> { <span class="var">id</span>: <span class="num">1</span> })\n        <span class="cm">// liberado de uma só vez, 0 pauses</span>\n    }\n}`,
            delayAfter: 5000
        }
    ];

    let currentPhase = 0;

    const simulateTyping = async (htmlString, container) => {
        container.innerHTML = '';
        let i = 0;
        let currentHTML = "";

        return new Promise(resolve => {
            const typer = setInterval(() => {
                if (i >= htmlString.length) {
                    clearInterval(typer);
                    resolve();
                    return;
                }

                // Jump over HTML tags exactly as one frame to preserve styling instantly
                if (htmlString.charAt(i) === '<') {
                    const tagEnd = htmlString.indexOf('>', i);
                    if (tagEnd !== -1) {
                        currentHTML += htmlString.substring(i, tagEnd + 1);
                        i = tagEnd + 1;
                    } else {
                        currentHTML += htmlString.charAt(i);
                        i++;
                    }
                } else {
                    currentHTML += htmlString.charAt(i);
                    i++;
                }

                container.innerHTML = currentHTML;
            }, 18); // typing speed
        });
    };

    const runLoop = async () => {
        while (true) {
            const phase = phases[currentPhase];

            // Glitch effect trigger
            editorWrapper.classList.remove('glitch-anim');
            void editorWrapper.offsetWidth; // force reflow
            editorWrapper.classList.add('glitch-anim');

            await simulateTyping(phase.text, editorCode);
            await new Promise(r => setTimeout(r, phase.delayAfter));

            // erase effect before next
            editorCode.innerHTML = '';

            currentPhase = (currentPhase + 1) % phases.length;
        }
    };

    // Start after 1 second
    setTimeout(runLoop, 1000);
}

// — Install command copy button ——————————————————
function copyInstall() {
    const code = document.querySelector('.hero__install-code');
    const btn  = document.querySelector('.hero__install-copy');
    if (!code || !btn) return;
    navigator.clipboard.writeText(code.textContent.trim()).then(() => {
        btn.classList.add('copied');
        btn.innerHTML = '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>';
        setTimeout(() => {
            btn.classList.remove('copied');
            btn.innerHTML = '<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x=\"9\" y=\"9\" width=\"13\" height=\"13\" rx=\"2\"/><path d=\"M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1\"/></svg>';
        }, 2000);
    });
}
