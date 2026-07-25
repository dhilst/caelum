// Upgrades static `.lum` code blocks into live CodeMirror editors backed by
// caelum-wasm. Loaded as an ES module by Sphinx (see conf.py `html_js_files`).
// It sits in _static/, so the sibling bundles resolve as `./caelum-editor.js`
// and `./caelum_wasm.js`.
//
// Degrades gracefully: if the wasm / editor bundles are missing (e.g. a docs
// build that skipped the asset copy), the original Pygments-highlighted blocks
// are left untouched.

const EDITOR_URL = new URL("./caelum-editor.js", import.meta.url).href;
const WASM_URL = new URL("./caelum_wasm.js", import.meta.url).href;

// A block is worth wiring to the checker when it declares a checkable property;
// other `.lum` snippets (type/var fragments) become editable, highlighted
// editors without a Check button.
function isCheckable(src) {
  return /\b(property|invalid)\b/.test(src);
}

function blockSource(block) {
  const pre = block.querySelector("pre");
  const text = pre ? pre.textContent : block.textContent;
  return text.replace(/\n$/, "");
}

// Sphinx's doctools.js installs global single-key shortcuts on `document`
// (`/` focuses search; ← / → page-navigate) and only exempts
// <input>/<textarea>/<select>. CodeMirror's editable area is a contenteditable
// <div>, so those shortcuts fire mid-typing. Stopping keydown events from
// bubbling out of the mounted editor keeps them from reaching the document
// handler, while CodeMirror (whose handlers live on the inner .cm-content)
// still processes the key at the target first.
function shieldGlobalKeys(host) {
  host.addEventListener("keydown", (event) => event.stopPropagation());
}

let editorModulePromise = null;
function loadEditor() {
  if (!editorModulePromise) editorModulePromise = import(EDITOR_URL);
  return editorModulePromise;
}

// Load and instantiate the wasm module once, then wrap it in the `{ check }`
// shape the editor expects. The page owns the `check_spec` string/opts contract
// (synchronous varisat "explicit" engine — no cross-origin isolation needed), so
// the editor stays agnostic of it.
let wasmPromise = null;
function loadWasm() {
  if (!wasmPromise) {
    wasmPromise = import(WASM_URL).then(async (mod) => {
      await mod.default(); // instantiate the .wasm
      return {
        check: (source) =>
          JSON.parse(mod.check_spec(source, JSON.stringify({ engine: "explicit" }))),
      };
    });
  }
  return wasmPromise;
}

async function upgradeCodeBlocks(mountCaelumEditor) {
  const blocks = Array.from(document.querySelectorAll(".highlight-lum"));
  if (blocks.length === 0) return;

  const anyCheckable = blocks.some((b) => isCheckable(blockSource(b)));
  const wasm = anyCheckable ? await loadWasm().catch(() => null) : null;

  for (const block of blocks) {
    const src = blockSource(block);
    const checkable = wasm && isCheckable(src);
    const wrapper = document.createElement("div");
    mountCaelumEditor(wrapper, {
      doc: src,
      wasm: checkable ? wasm : undefined,
    });
    shieldGlobalKeys(wrapper);
    block.replaceWith(wrapper);
  }
}

async function upgradePlayground(mountCaelumEditor) {
  const host = document.getElementById("caelum-playground");
  if (!host) return;

  // A `?q=<url>` query parameter overrides the page's default seed, so any
  // `.lum` file — a relative path under _static/, or an absolute URL (e.g. a
  // raw.githubusercontent.com link) — can be opened straight in the playground.
  // This makes specs shareable: whatever is in the box goes into `?q=`, and a
  // `?q=` on load is fetched automatically.
  const params = new URLSearchParams(window.location.search);
  const qParam = params.get("q");
  const seedUrl = qParam || host.getAttribute("data-seed-url");

  let doc = host.getAttribute("data-seed") || "";
  if (seedUrl) {
    try {
      // An absolute URL ignores the base; a relative one resolves against the
      // page. Cross-origin fetches (e.g. raw.githubusercontent.com, which sends
      // `Access-Control-Allow-Origin: *`) work; a host without CORS headers is
      // blocked and surfaced below.
      const resp = await fetch(new URL(seedUrl, document.baseURI));
      if (resp.ok) doc = await resp.text();
      else if (qParam) doc = "// Could not load " + seedUrl + " — HTTP " + resp.status + "\n";
    } catch (_e) {
      if (qParam) doc = "// Could not load " + seedUrl + " (network error or blocked by CORS)\n";
    }
  }

  const wasm = await loadWasm().catch(() => null);
  host.textContent = "";
  mountCaelumEditor(host, { doc, wasm: wasm || undefined });
  shieldGlobalKeys(host);

  // Wire the "Load from URL" control (see playground.rst): typing a URL and
  // pressing Load / Enter reloads the page with `?q=<url>`, so the resulting
  // address is itself a shareable link to that spec.
  const urlInput = document.getElementById("caelum-load-url");
  if (urlInput) {
    if (qParam) urlInput.value = qParam;
    const go = () => {
      const v = urlInput.value.trim();
      if (v) window.location.search = "?q=" + encodeURIComponent(v);
    };
    const loadBtn = document.getElementById("caelum-load-btn");
    if (loadBtn) loadBtn.addEventListener("click", go);
    urlInput.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        go();
      }
    });
  }
}

async function main() {
  const hasBlocks = document.querySelector(".highlight-lum");
  const hasPlayground = document.getElementById("caelum-playground");
  if (!hasBlocks && !hasPlayground) return;

  let mod;
  try {
    mod = await loadEditor();
  } catch (_e) {
    // Editor bundle unavailable — leave static highlighting in place.
    return;
  }
  const { mountCaelumEditor } = mod;

  await upgradeCodeBlocks(mountCaelumEditor);
  await upgradePlayground(mountCaelumEditor);
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", main);
} else {
  main();
}
