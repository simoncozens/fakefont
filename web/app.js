/* ===========================================================================
   How Big Is A Font? — UI controller
   ===========================================================================

   The font itself is built by the `fakefont-web` crate (see ../fakefont-web),
   whose wasm-bindgen output this file loads from ./pkg/. Build it first:

     wasm-pack build fakefont-web --target web --out-dir ../web/pkg

   The API it consumes:

     new FakeFont(latinCoverage, scripts)          // "full" | "core" | "kernel",
                                                   // then the script tile values
     font.addWeightAxis(min, max)                  // only when that axis is enabled
     font.addWidthAxis(min, max)
     font.addArbitraryAxis(tag, name, low, high, default, metrics, kerning)
     font.fillOutMasters(kerning, advanceWidths)
     const bytes = font.compile()                  // Uint8Array
     const tables = tableStats(bytes)              // Map<string, number>

   `scripts` is the list of checked `input[name="script"]` values, which the
   wasm module accepts verbatim: greek, cyrillic, cjk-basic, devanagari,
   bengali, standard-arabic, farsi-urdu, thai, tamil, telugu, kannada.
*/

/**
 * @typedef {object} CustomAxis
 * @property {string} tag
 * @property {string} name
 * @property {number} low
 * @property {number} default
 * @property {number} high
 * @property {boolean} affectsMetrics
 * @property {boolean} affectsKerning
 */

/**
 * @typedef {object} Options
 * @property {"full"|"core"|"kernel"} latinCoverage
 * @property {string[]} scripts
 * @property {Record<string, {min: number, max: number, preset: string} | null>} axes
 * @property {CustomAxis[]} customAxes
 * @property {boolean} kerning
 * @property {boolean} advanceWidths
 */

/** Axes offered in the designspace pane. Order is the order they are applied. */
const AXES = [
  { tag: "wght", name: "Weight" },
  { tag: "wdth", name: "Width" },
];

/** An axis tag is four printable ASCII bytes. */
const AXIS_TAG_PATTERN = /^[\x20-\x7e]{4}$/;

/** Values a newly added custom axis starts with. */
const NEW_AXIS = { low: 0, default: 0, high: 100 };

/**
 * The two halves of a build, as the spinner names them. Generating slices the
 * chosen scripts out of the source font and expands the designspace into
 * masters; compiling hands that font to fontc.
 */
const GENERATING = "Generating…";
const COMPILING = "Compiling…";

// ------------------------------------------------------------------ elements

const els = {
  compileButton: document.getElementById("compile-button"),
  compileLabel: document.querySelector("#compile-button .btn-compile-label"),
  results: document.getElementById("results"),
  body: document.getElementById("results-body"),
  stats: document.querySelector(".js-results-stats"),
  notice: document.getElementById("results-notice"),
  totalBytes: document.querySelectorAll(".js-total-bytes"),
  totalHuman: document.querySelector(".js-total-human"),
  tableBody: document.querySelector("#table-breakdown tbody"),
  download: document.getElementById("download-button"),
  addAxisButton: document.getElementById("add-axis-button"),
  axesWrap: document.getElementById("axes-wrap"),
  axesBody: document.getElementById("axes-body"),
  axesHint: document.getElementById("axes-hint"),
  axesError: document.getElementById("axes-error"),
  axisRowTemplate: document.getElementById("axis-row-template"),
};

let downloadUrl = null;
/** True while a compile is running, so validation can't re-enable the button. */
let busy = false;
/** False when a custom axis is not usable as-is. */
let axesValid = true;

// -------------------------------------------------------------- state model

/** The weight and width presets, as chosen with the tiles. */
function readPresetAxes() {
  const axes = {};
  for (const { tag } of AXES) {
    const checked = document.querySelector(`input[name="axis-${tag}"]:checked`);
    axes[tag] =
      checked && checked.value !== "none"
        ? {
            min: Number(checked.dataset.min),
            max: Number(checked.dataset.max),
            preset: checked.value,
          }
        : null;
  }
  return axes;
}

/** The raw contents of the custom axis table. */
function readCustomAxes() {
  return axisRows().map((row) => {
    const text = (selector) => row.querySelector(selector).value.trim();
    const number = (selector) => row.querySelector(selector).valueAsNumber;
    return {
      tag: text(".axis-tag-input"),
      name: text(".axis-name-input"),
      low: number(".axis-low-input"),
      default: number(".axis-default-input"),
      high: number(".axis-high-input"),
      affectsMetrics: row.querySelector(".axis-metrics-input").checked,
      affectsKerning: row.querySelector(".axis-kerning-input").checked,
    };
  });
}

/**
 * Read the whole UI into a plain options object.
 * @returns {Options}
 */
export function collectOptions() {
  const latin = document.querySelector('input[name="latin-coverage"]:checked');

  return {
    latinCoverage: latin ? latin.value : "full",
    scripts: Array.from(
      document.querySelectorAll('input[name="script"]:checked'),
    ).map((input) => input.value),
    axes: readPresetAxes(),
    customAxes: readCustomAxes(),
    kerning: document.getElementById("adjust-kerning").checked,
    advanceWidths: document.getElementById("adjust-advance-widths").checked,
  };
}

/** Cached wasm module promise, so the (large) module is instantiated once. */
let wasmModule = null;

function loadWasm() {
  if (!wasmModule) {
    wasmModule = import("./pkg/fakefont_web.js").then(async (wasm) => {
      await wasm.default();
      return wasm;
    });
  }
  return wasmModule;
}

/**
 * Turn the UI state into a font and compile it.
 * @param {Options} options
 * @returns {Promise<{fontBytes: Uint8Array|null, tables: Map<string, number>, notice?: string}>}
 */
async function compileFont(options) {
  const wasm = await loadWasm();

  // Generating. Every subset is chosen at construction, and the tile values are
  // the names the wasm module expects, so the checked boxes can be passed
  // straight through.
  const font = new wasm.FakeFont(options.latinCoverage, options.scripts);
  if (options.axes.wght)
    font.addWeightAxis(options.axes.wght.min, options.axes.wght.max);
  if (options.axes.wdth)
    font.addWidthAxis(options.axes.wdth.min, options.axes.wdth.max);
  // The library takes low, high, default — note the order.
  for (const axis of options.customAxes) {
    font.addArbitraryAxis(
      axis.tag,
      axis.name || axis.tag,
      axis.low,
      axis.high,
      axis.default,
      axis.affectsMetrics,
      axis.affectsKerning,
    );
  }
  font.fillOutMasters(options.kerning, options.advanceWidths);

  // Read these before compiling. compile() consumes the font so that the library
  // doesn't have to clone it, which leaves the object unusable afterwards. They
  // still come after fillOutMasters, so the counts reflect the built designspace.
  const masters = font.masterCount();
  const glyphs = font.glyphCount();

  // Compiling. The font has been generated, and from here on it is fontc's.
  await setPhase(COMPILING);
  const fontBytes = font.compile();
  return {
    fontBytes,
    tables: wasm.tableStats(fontBytes),
    masters,
    glyphs,
  };
}

// ------------------------------------------------------------- custom axes

/** The `<tr>`s currently in the custom axis table. */
function axisRows() {
  return Array.from(els.axesBody.querySelectorAll("tr"));
}

/**
 * Append an axis row.
 * @param {Partial<CustomAxis>} [values]
 * @param {boolean} [focus] Leave the tag focused, unless we are restoring.
 */
function addAxisRow(values = {}, focus = true) {
  const row = els.axisRowTemplate.content.firstElementChild.cloneNode(true);
  const field = (selector) => row.querySelector(selector);

  field(".axis-tag-input").value = values.tag ?? "";
  field(".axis-name-input").value = values.name ?? "";
  field(".axis-low-input").value = String(values.low ?? NEW_AXIS.low);
  field(".axis-default-input").value = String(
    values.default ?? NEW_AXIS.default,
  );
  field(".axis-high-input").value = String(values.high ?? NEW_AXIS.high);
  field(".axis-metrics-input").checked = values.affectsMetrics ?? true;
  field(".axis-kerning-input").checked = values.affectsKerning ?? true;

  els.axesBody.append(row);
  refreshAxes();
  saveSettingsSoon();
  if (focus) field(".axis-tag-input").focus();
  return row;
}

function removeAxisRow(row) {
  row.remove();
  refreshAxes();
  saveSettingsSoon();
}

/**
 * Check every custom axis: tags the library would reject, duplicate tags, and
 * ranges that don't make sense.
 *
 * @returns {{rows: CustomAxis[], problems: {tag: string|null, range: string|null}[], messages: string[], valid: boolean}}
 */
function validateAxes() {
  const rows = readCustomAxes();
  const problems = rows.map(() => ({ tag: null, range: null }));
  const messages = [];

  // Tags the preset axes are already using, so a custom axis can't collide.
  const claimed = new Map();
  const presets = readPresetAxes();
  for (const { tag, name } of AXES) {
    if (presets[tag]) claimed.set(tag, `${name.toLowerCase()} axis`);
  }

  rows.forEach((row, index) => {
    const at = (text) => `Row ${index + 1}: ${text}`;

    if (!AXIS_TAG_PATTERN.test(row.tag)) {
      problems[index].tag =
        "the tag must be exactly four printable ASCII characters";
    } else if (claimed.has(row.tag)) {
      problems[index].tag =
        `the tag "${row.tag}" is already used by the ${claimed.get(row.tag)}`;
    } else {
      claimed.set(row.tag, `row ${index + 1}`);
    }

    if (![row.low, row.default, row.high].every(Number.isFinite)) {
      problems[index].range = "low, default and high must all be numbers";
    } else if (row.low >= row.high) {
      problems[index].range = "low must be less than high";
    } else if (row.default < row.low || row.default > row.high) {
      problems[index].range = "the default must lie between low and high";
    }

    for (const problem of [problems[index].tag, problems[index].range]) {
      if (problem) messages.push(at(problem));
    }
  });

  return { rows, problems, messages, valid: messages.length === 0 };
}

/** Paint the validation state onto the rows, the message line and the button. */
function refreshAxes() {
  const hasRows = axisRows().length > 0;
  els.axesWrap.hidden = !hasRows;
  els.axesHint.hidden = hasRows;

  const { problems, messages } = validateAxes();
  axisRows().forEach((row, index) => {
    markInvalid(row.querySelector(".axis-tag-input"), problems[index].tag);
    for (const selector of [
      ".axis-low-input",
      ".axis-default-input",
      ".axis-high-input",
    ]) {
      markInvalid(row.querySelector(selector), problems[index].range);
    }
  });

  els.axesError.textContent = messages.join(" ");
  els.axesError.hidden = messages.length === 0;

  axesValid = messages.length === 0;
  refreshCompileButton();
}

/**
 * @param {HTMLInputElement} input
 * @param {string|null} message
 */
function markInvalid(input, message) {
  input.classList.toggle("is-invalid", Boolean(message));
  if (message) {
    input.setAttribute("aria-invalid", "true");
    input.title = message;
  } else {
    input.removeAttribute("aria-invalid");
    input.removeAttribute("title");
  }
}

/** Disabled while compiling, and while the custom axes need fixing. */
function refreshCompileButton() {
  els.compileButton.disabled = busy || !axesValid;
  els.compileButton.title = axesValid
    ? ""
    : "Fix the custom axes before compiling";
}

// ------------------------------------------------------------------ compile

async function run() {
  // Belt and braces: the button is disabled in this state, but bad axes would
  // panic inside the library and abort the wasm module, so refuse to start.
  const { valid, messages } = validateAxes();
  if (!valid) {
    els.body.hidden = true;
    showNotice(`Custom axes need fixing — ${messages.join(" ")}`, true);
    reveal();
    return;
  }

  setBusy(true, GENERATING);

  try {
    const options = collectOptions();
    // Let the browser paint the spinner before we hand over to wasm, which
    // blocks the main thread for the rest of the build.
    await yieldToBrowser();
    const result = await compileFont(options);
    renderResults(result);
    reveal();
  } catch (error) {
    console.error(error);
    // Don't leave the previous run's figures sitting under an error notice.
    els.body.hidden = true;
    showNotice(`Something went wrong: ${error?.message ?? error}`, true);
    reveal();
  } finally {
    setBusy(false);
  }
}

/** Show the results panel and bring it into view. */
function reveal() {
  els.results.hidden = false;
  els.results.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

/**
 * Hand the browser a frame so it can paint before a long blocking call. The
 * timeout is a fallback for background tabs, where frames are throttled away.
 * @returns {Promise<void>}
 */
function yieldToBrowser() {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, 120);
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        clearTimeout(timer);
        resolve();
      });
    });
  });
}

/**
 * While busy the results panel is hidden rather than left showing the previous
 * run's numbers, and the button carries the spinner.
 * @param {boolean} state
 * @param {string} [label] What the spinner says while it is busy.
 */
function setBusy(state, label) {
  busy = state;
  els.compileButton.classList.toggle("is-busy", state);
  els.compileButton.setAttribute("aria-busy", String(state));
  els.compileLabel.textContent = state ? label : "Compile font";
  refreshCompileButton();

  if (state) {
    els.results.hidden = true;
    els.body.hidden = false;
    els.notice.hidden = true;
    setDownload(null);
  }
}

/**
 * Rename the current phase and give the browser a frame to paint it. Each phase
 * blocks the main thread until it is done, so without the yield the label would
 * only ever change after the work it describes had finished.
 * @param {string} label
 * @returns {Promise<void>}
 */
async function setPhase(label) {
  els.compileLabel.textContent = label;
  await yieldToBrowser();
}

// ------------------------------------------------------------------ results

/**
 * Accepts tables as a plain object or as a `Map`, because wasm-bindgen hands
 * back a `Map` when the Rust side returns a `HashMap`.
 * @param {Record<string, number>|Map<string, number>|null|undefined} tables
 * @returns {{tag: string, bytes: number}[]}
 */
function normaliseTables(tables) {
  const entries =
    tables instanceof Map
      ? Array.from(tables.entries())
      : Object.entries(tables ?? {});
  return entries
    .map(([tag, bytes]) => ({ tag: String(tag), bytes: Number(bytes) }))
    .filter((row) => Number.isFinite(row.bytes))
    .sort((a, b) => b.bytes - a.bytes);
}

/**
 * The line above the size: how much font was built to get it.
 * @param {number|undefined} masters
 * @param {number|undefined} glyphs
 */
function renderStats(masters, glyphs) {
  const parts = [];
  if (Number.isFinite(masters)) {
    parts.push(
      `${masters.toLocaleString("en-US")} ${masters === 1 ? "master" : "masters"}`,
    );
  }
  if (Number.isFinite(glyphs)) {
    parts.push(
      `${glyphs.toLocaleString("en-US")} ${glyphs === 1 ? "glyph" : "glyphs"}`,
    );
  }
  els.stats.textContent = parts.join(" x ") + " ≈ ";
  els.stats.hidden = parts.length === 0;
}

/**
 * @param {{fontBytes: Uint8Array|null, tables: Record<string, number>|Map<string, number>, masters?: number, glyphs?: number, notice?: string}} result
 */
export function renderResults({ fontBytes, tables, masters, glyphs, notice }) {
  const rows = normaliseTables(tables);
  const total = rows.reduce((sum, row) => sum + row.bytes, 0);

  renderStats(masters, glyphs);

  for (const el of els.totalBytes) {
    el.textContent = total.toLocaleString("en-US");
  }
  els.totalHuman.textContent = humanBytes(total);

  els.tableBody.replaceChildren();

  if (rows.length === 0) {
    const tr = document.createElement("tr");
    const td = document.createElement("td");
    td.colSpan = 2;
    td.className = "table-empty";
    td.textContent = "No table data yet.";
    tr.append(td);
    els.tableBody.append(tr);
  } else {
    for (const { tag, bytes } of rows) {
      const tr = document.createElement("tr");

      const name = document.createElement("td");
      const code = document.createElement("span");
      code.className = "tag";
      code.textContent = tag;
      name.append(code);

      const size = document.createElement("td");
      size.className = "text-end js-bytes";
      size.textContent = bytes.toLocaleString("en-US");

      tr.append(name, size);
      els.tableBody.append(tr);
    }
  }

  setDownload(fontBytes);
  if (notice) {
    showNotice(notice, false);
  } else {
    els.notice.hidden = true;
  }
}

/**
 * @param {Uint8Array|null} fontBytes
 */
function setDownload(fontBytes) {
  if (downloadUrl) {
    URL.revokeObjectURL(downloadUrl);
    downloadUrl = null;
  }

  if (!fontBytes || fontBytes.length === 0) {
    els.download.setAttribute("aria-disabled", "true");
    els.download.classList.add("disabled");
    els.download.removeAttribute("href");
    els.download.title = "Nothing to download yet";
    return;
  }

  downloadUrl = URL.createObjectURL(
    new Blob([fontBytes], { type: "font/ttf" }),
  );
  els.download.href = downloadUrl;
  els.download.download = "fakefont.ttf";
  els.download.classList.remove("disabled");
  els.download.removeAttribute("aria-disabled");
  els.download.title = "Download the compiled font";
}

/**
 * @param {string} message
 * @param {boolean} [isError]
 */
function showNotice(message, isError = false) {
  els.notice.textContent = message;
  els.notice.classList.toggle("results-notice-error", isError);
  els.notice.hidden = false;
}

/**
 * @param {number} bytes
 * @returns {string}
 */
function humanBytes(bytes) {
  if (bytes < 1024) return `${bytes} bytes`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  return `${(kb / 1024).toFixed(2)} MB`;
}

// ------------------------------------------------------------------ storage

/**
 * Where the choices live between visits. The version suffix means a future
 * change to the shape can start from scratch instead of misreading old data.
 */
const STORAGE_KEY = "how-big-is-a-font.settings.v1";

/** @returns {object|null} */
function readStoredSettings() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : null;
  } catch (error) {
    // Storage can be unavailable (private windows, blocked site data) and the
    // value can be junk. Either way, fall back to the defaults.
    console.warn("[how-big-is-a-font] could not read saved settings", error);
    return null;
  }
}

let saveTimer = null;

/** Remember the current choices, coalescing bursts of typing into one write. */
function saveSettingsSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(collectOptions()));
    } catch (error) {
      console.warn("[how-big-is-a-font] could not save settings", error);
    }
  }, 250);
}

/** Forget the saved choices. */
function clearSettings() {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch (error) {
    console.warn("[how-big-is-a-font] could not clear settings", error);
  }
}

/** Check the radio in the group `name` whose value matches, if there is one. */
function selectRadio(name, value) {
  if (typeof value !== "string") return;
  const input = Array.from(
    document.querySelectorAll(`input[name="${name}"]`),
  ).find((candidate) => candidate.value === value);
  if (input) input.checked = true;
}

/**
 * A stored number as an input value. `null` is how JSON records an input that
 * was left empty, and it goes back to being empty rather than to the default.
 */
function storedNumber(value, fallback) {
  if (typeof value === "number") return String(value);
  if (value === null) return "";
  return String(fallback);
}

/**
 * Put back the choices from the last visit. Anything missing, unrecognised or
 * malformed is skipped, so a stale record can never leave the page stuck.
 * @param {any} stored
 */
function applySettings(stored) {
  if (!stored || typeof stored !== "object") return;

  selectRadio("latin-coverage", stored.latinCoverage);

  if (Array.isArray(stored.scripts)) {
    for (const box of document.querySelectorAll('input[name="script"]')) {
      box.checked = stored.scripts.includes(box.value);
    }
  }

  for (const { tag } of AXES) {
    selectRadio(`axis-${tag}`, stored.axes?.[tag]?.preset ?? "none");
  }

  if (Array.isArray(stored.customAxes)) {
    els.axesBody.replaceChildren();
    for (const axis of stored.customAxes) {
      if (!axis || typeof axis !== "object") continue;
      addAxisRow(
        {
          tag: axis.tag,
          name: axis.name,
          low: storedNumber(axis.low, NEW_AXIS.low),
          default: storedNumber(axis.default, NEW_AXIS.default),
          high: storedNumber(axis.high, NEW_AXIS.high),
          affectsMetrics: axis.affectsMetrics,
          affectsKerning: axis.affectsKerning,
        },
        false,
      );
    }
  }

  if (typeof stored.kerning === "boolean") {
    document.getElementById("adjust-kerning").checked = stored.kerning;
  }
  if (typeof stored.advanceWidths === "boolean") {
    document.getElementById("adjust-advance-widths").checked =
      stored.advanceWidths;
  }
}

// -------------------------------------------------------------------- wiring

els.compileButton.addEventListener("click", run);

els.addAxisButton.addEventListener("click", () => addAxisRow());

// Rows come and go, so the table's own listeners are delegated.
els.axesBody.addEventListener("input", () => {
  refreshAxes();
  saveSettingsSoon();
});
els.axesBody.addEventListener("click", (event) => {
  const button = event.target.closest(".btn-remove-axis");
  if (button) removeAxisRow(button.closest("tr"));
});

// Switching a preset axis on claims its tag, which can invalidate a custom axis.
document.addEventListener("change", (event) => {
  if (
    event.target instanceof HTMLInputElement &&
    event.target.name?.startsWith("axis-")
  ) {
    refreshAxes();
  }
  saveSettingsSoon();
});

// Keep the download button from doing anything when it is inert.
els.download.addEventListener("click", (event) => {
  if (els.download.classList.contains("disabled")) event.preventDefault();
});

setDownload(null);
els.notice.hidden = true;
applySettings(readStoredSettings());
refreshAxes();

// Handy when poking at this from the devtools console.
globalThis.HowBigIsAFont = {
  addAxisRow,
  clearSettings,
  collectOptions,
  renderResults,
  run,
  validateAxes,
};
