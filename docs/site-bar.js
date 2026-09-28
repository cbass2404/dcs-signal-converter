// The bar across the top of every page, so it is written once. A page puts
// <nav class="site-bar" data-here="..."> first in its body, with anything of
// its own inside (the language page's Contents button), and loads this
// straight after it. data-here is "home", "language" or "gauges": the page's
// own button is lit, and is not a link, since it would only reload the page.
(() => {
  "use strict";
  const REPO = "https://github.com/cbass2404/dcs-signal-converter";
  const bar = document.querySelector("nav.site-bar");
  if (!bar) return;
  const here = bar.dataset.here;
  const own = [...bar.childNodes];

  const el = (tag, attrs, html) => {
    const n = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) if (v != null) n.setAttribute(k, v);
    if (html != null) n.innerHTML = html;
    return n;
  };
  // A link, or on its own page the same button with nowhere to go.
  const to = (key, href) => (here === key ? { "aria-current": "page" } : { href });
  const page = (key, href, label) => el("a", { class: "nav", ...to(key, href) }, label);

  const inner = el("div", { class: "site-bar-in" });
  inner.append(
    el(
      "a",
      { class: "site-home", ...to("home", "./") },
      '<img src="img/icon.svg" alt="" width="26" height="26" /><span>DCS Signal Converter</span>',
    ),
    ...own,
  );

  const links = el("div", { class: "site-links", id: "site-links" });
  links.append(
    page("language", "language.html", "Profile language"),
    page("gauges", "gauges.html", "Uneven gauges"),
    el("a", { class: "cta", href: `${REPO}/releases/latest` }, "Download"),
    el(
      "a",
      { class: "cta ghost", href: "https://www.buymeacoffee.com/cbass2404", target: "_blank", rel: "noopener noreferrer" },
      '<svg viewBox="0 0 24 24" fill="none" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' +
        '<path d="M4.2 9h12.6v5.6a5 5 0 0 1-5 5H9.2a5 5 0 0 1-5-5V9Z" /><path d="M16.8 10.2h1.5a2.4 2.4 0 0 1 0 4.8h-1.5" />' +
        '<path d="M8.4 2.6v2.3M12.6 2.6v2.3" /></svg>Buy me a coffee',
    ),
  );

  // Only drawn on a narrow window, where the links fold under it.
  const menu = el(
    "button",
    { type: "button", class: "site-menu", "aria-controls": "site-links", "aria-expanded": "false", "aria-label": "Menu" },
    '<svg viewBox="0 0 24 24" fill="none" stroke-width="2" stroke-linecap="round" aria-hidden="true">' +
      '<path class="bars" d="M4 7h16M4 12h16M4 17h16" /><path class="x" d="M6 6l12 12M18 6L6 18" /></svg>',
  );
  const show = (open) => {
    links.classList.toggle("open", open);
    menu.setAttribute("aria-expanded", String(open));
  };
  menu.addEventListener("click", () => show(!links.classList.contains("open")));
  document.addEventListener("click", (e) => {
    if (!e.target.closest(".site-menu, .site-links")) show(false);
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && links.classList.contains("open")) {
      show(false);
      menu.focus();
    }
  });

  inner.append(menu, links);
  bar.replaceChildren(inner);
})();
