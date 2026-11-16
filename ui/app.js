const { invoke } = window.__TAURI__.core

let shellRoutes = []
let shellInfo = null
let lastMeasure = null
let beforeBench = null
let afterBench = null
let lastVerdict = null
let profilesState = null
let homeCache = null

const parseRouteId = () => {
  const hash = window.location.hash.replace(/^#\/?/, "")
  return hash || "home"
}

const metricValue = (metric) => {
  if (!metric || metric.status !== "available") {
    return null
  }
  return metric.value
}

const formatMetric = (metric) => {
  if (!metric || !metric.status) {
    return "n/d"
  }
  if (metric.status === "available") {
    const unit = metric.unit ? ` ${metric.unit}` : ""
    return `${metric.value}${unit}`
  }
  return "n/d"
}

const formatTextMetric = (metric) => {
  if (!metric || metric.status !== "available") {
    return "n/d"
  }
  return metric.value
}

const formatSpread = (spread) => {
  if (!spread) {
    return "n/d"
  }
  return `mediana ${spread.median.toFixed(1)} (IQR ${spread.iqr.toFixed(1)}; ${spread.min.toFixed(1)}--${spread.max.toFixed(1)})`
}

const formatBytes = (value) => {
  if (value === undefined || value === null) {
    return "n/d"
  }
  if (value < 1024) {
    return `${value} B`
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KiB`
  }
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MiB`
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GiB`
}

const downloadText = (filename, text, mime) => {
  const blob = new Blob([text], { type: mime })
  const url = URL.createObjectURL(blob)
  const link = document.createElement("a")
  link.href = url
  link.download = filename
  link.rel = "noopener"
  document.body.appendChild(link)
  link.click()
  link.remove()
  URL.revokeObjectURL(url)
}

const el = (tag, className, text) => {
  const node = document.createElement(tag)
  if (className) {
    node.className = className
  }
  if (text !== undefined && text !== null) {
    node.textContent = text
  }
  return node
}

const field = (labelText, input) => {
  const wrap = el("label", "field")
  wrap.appendChild(el("span", "field-label", labelText))
  wrap.appendChild(input)
  return wrap
}

const button = (label, onClick, opts = {}) => {
  const btn = el("button", opts.className || "btn")
  btn.type = "button"
  btn.textContent = label
  if (opts.ariaLabel) {
    btn.setAttribute("aria-label", opts.ariaLabel)
  }
  btn.addEventListener("click", onClick)
  btn.addEventListener("keydown", (event) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault()
      onClick(event)
    }
  })
  return btn
}

const statTile = (label, value, hint) => {
  const tile = el("div", "stat-tile")
  tile.appendChild(el("span", "stat-tile__label", label))
  tile.appendChild(el("span", "stat-tile__value", value))
  if (hint) {
    tile.appendChild(el("span", "stat-tile__hint", hint))
  }
  return tile
}

const updateStepStrip = (activeId) => {
  const map = {
    home: null,
    measure: "measure",
    actions: "actions",
    benchmark: "benchmark",
  }
  const current = map[activeId] || null
  document.querySelectorAll(".steps li").forEach((li) => {
    const step = li.getAttribute("data-step")
    li.classList.toggle("is-active", step === current)
  })
}

const renderNav = (routes, activeId) => {
  const nav = document.querySelector("#nav")
  nav.replaceChildren()
  for (const route of routes) {
    const link = document.createElement("a")
    link.href = route.hash_path
    link.textContent = route.title
    link.dataset.route = route.id
    link.tabIndex = 0
    if (route.id === activeId) {
      link.setAttribute("aria-current", "page")
    }
    link.addEventListener("click", (event) => {
      event.preventDefault()
      handleNavigate(route.id)
    })
    link.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault()
        handleNavigate(route.id)
      }
    })
    nav.appendChild(link)
  }
}

const scoreFromOverview = (overview, measure) => {
  const parts = []
  const cpu = overview?.hogs?.hogs?.[0]?.cpu_percent
  if (typeof cpu === "number") {
    parts.push(Math.max(0, 100 - Math.min(cpu, 100)))
  }
  const thermal = overview?.thermal
  if (thermal && typeof thermal.cpu_temp_celsius === "number") {
    const t = thermal.cpu_temp_celsius
    parts.push(t >= 90 ? 35 : t >= 80 ? 55 : t >= 70 ? 75 : 92)
  }
  if (thermal?.level === "high" || thermal?.level === "critical") {
    parts.push(40)
  } else if (thermal?.level === "ok" || thermal?.level === "normal") {
    parts.push(88)
  }
  const startupOn = (overview?.startup?.rows || []).filter((r) => r.enabled).length
  if (overview?.startup?.rows?.length) {
    parts.push(Math.max(30, 100 - startupOn * 6))
  }
  const cleanup = overview?.cleanup?.total_bytes
  if (typeof cleanup === "number") {
    const gib = cleanup / (1024 * 1024 * 1024)
    parts.push(gib > 8 ? 45 : gib > 2 ? 65 : 90)
  }
  const fps = metricValue(measure?.session?.frame?.avg_fps)
  if (typeof fps === "number") {
    parts.push(fps >= 60 ? 95 : fps >= 45 ? 75 : fps >= 30 ? 55 : 35)
  }
  const cpuUse = metricValue(measure?.session?.hardware?.cpu_usage_percent)
  if (typeof cpuUse === "number") {
    parts.push(Math.max(0, 100 - cpuUse))
  }
  if (!parts.length) {
    return { score: null, samples: 0 }
  }
  const score = Math.round(parts.reduce((a, b) => a + b, 0) / parts.length)
  return { score, samples: parts.length }
}

const renderMetricTable = (session) => {
  const table = el("table", "metrics")
  table.setAttribute("aria-label", "Risultati misurazione")
  const body = document.createElement("tbody")
  const rows = [
    ["FPS medio", formatMetric(session.frame.avg_fps)],
    ["1% low FPS", formatMetric(session.frame.low_1pct_fps)],
    ["0.1% low FPS", formatMetric(session.frame.low_0_1pct_fps)],
    ["Varianza frame", formatMetric(session.frame.frame_time_variance_ms2)],
    ["CPU", formatMetric(session.hardware.cpu_usage_percent)],
    ["GPU", formatMetric(session.hardware.gpu_usage_percent)],
    ["RAM", formatMetric(session.hardware.ram_used_bytes)],
    ["VRAM", formatMetric(session.hardware.vram_used_bytes)],
    ["Temp CPU", formatMetric(session.hardware.cpu_temp_celsius)],
    ["Temp GPU", formatMetric(session.hardware.gpu_temp_celsius)],
    ["Throttling", formatTextMetric(session.hardware.throttling)],
    ["Disco lettura", formatMetric(session.disk.disk_read_bytes_per_sec)],
    ["Disco scrittura", formatMetric(session.disk.disk_write_bytes_per_sec)],
    ["Coda disco", formatMetric(session.disk.disk_queue_depth)],
    ["Host rete", formatTextMetric(session.network.network_host)],
    ["Latenza", formatMetric(session.network.network_latency_ms)],
    ["Jitter", formatMetric(session.network.network_jitter_ms)],
    ["Packet loss", formatMetric(session.network.network_packet_loss_percent)],
  ]
  for (const [name, value] of rows) {
    const tr = document.createElement("tr")
    tr.appendChild(el("th", null, name))
    tr.appendChild(el("td", null, value))
    body.appendChild(tr)
  }
  table.appendChild(body)
  return table
}

const renderMeasureCards = (session) => {
  const grid = el("div", "stat-grid")
  grid.appendChild(
    statTile(
      "FPS medio",
      formatMetric(session.frame.avg_fps),
      "Solo se il probe frame e disponibile"
    )
  )
  grid.appendChild(statTile("1% low", formatMetric(session.frame.low_1pct_fps)))
  grid.appendChild(statTile("CPU", formatMetric(session.hardware.cpu_usage_percent)))
  grid.appendChild(statTile("GPU", formatMetric(session.hardware.gpu_usage_percent)))
  grid.appendChild(statTile("RAM", formatMetric(session.hardware.ram_used_bytes)))
  grid.appendChild(statTile("Temp CPU", formatMetric(session.hardware.cpu_temp_celsius)))
  return grid
}

const withTimeout = (promise, ms, label) => {
  let timer
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} timeout ${ms}ms`)), ms)
  })
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer))
}

const paintHomeShell = (dash, sideMessage) => {
  dash.replaceChildren()
  const ring = el("div", "score-ring")
  ring.style.setProperty("--pct", 0)
  const inner = el("div", "score-ring__inner")
  inner.appendChild(el("span", "score-ring__value", "--"))
  inner.appendChild(el("span", "score-ring__label", "salute"))
  ring.appendChild(inner)
  dash.appendChild(ring)
  const side = el("div")
  side.appendChild(el("p", null, sideMessage))
  const cta = el("div", "cta-row")
  cta.appendChild(
    button("1. Misura ora", () => handleNavigate("measure"), {
      ariaLabel: "Vai a Misura",
      className: "btn btn-large",
    })
  )
  cta.appendChild(
    button("2. Ottimizza", () => handleNavigate("actions"), {
      ariaLabel: "Vai a Ottimizza",
      className: "btn btn-large btn-secondary",
    })
  )
  cta.appendChild(
    button("3. Verifica", () => handleNavigate("benchmark"), {
      ariaLabel: "Vai a Verifica",
      className: "btn btn-large btn-secondary",
    })
  )
  side.appendChild(cta)
  dash.appendChild(side)
}

const renderHome = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Tre passi: misura lo stato attuale, applica solo modifiche spiegate e reversibili, poi verifica il guadagno sulla tua macchina."
    )
  )
  const status = el("p", "status", "Carico panoramica...")
  status.setAttribute("role", "status")
  const dash = el("div", "panel hero-dash")
  const results = el("div", "results")
  const skeleton = el("div", "skeleton")
  skeleton.setAttribute("aria-hidden", "true")
  results.appendChild(skeleton)
  paintHomeShell(
    dash,
    "Sto leggendo CPU, avvio e disco. Puoi gia andare a Misura."
  )
  root.appendChild(status)
  root.appendChild(dash)
  root.appendChild(results)

  try {
    const overview = await withTimeout(
      invoke("everyday_get_overview"),
      8000,
      "everyday_get_overview"
    )
    homeCache = overview
    const { score, samples } = scoreFromOverview(overview, lastMeasure)

    dash.replaceChildren()
    const ring = el("div", "score-ring")
    ring.style.setProperty("--pct", score == null ? 0 : score)
    const inner = el("div", "score-ring__inner")
    inner.appendChild(el("span", "score-ring__value", score == null ? "--" : `${score}%`))
    inner.appendChild(el("span", "score-ring__label", "salute"))
    ring.appendChild(inner)
    dash.appendChild(ring)

    const side = el("div")
    side.appendChild(
      el(
        "p",
        null,
        score == null
          ? "Score non ancora calcolabile: avvia una misura."
          : `Score da ${samples} indicatori reali (CPU, termiche, avvio, spazio, ultima misura). Non e un claim FPS.`
      )
    )
    const cta = el("div", "cta-row")
    cta.appendChild(
      button("1. Misura ora", () => handleNavigate("measure"), {
        ariaLabel: "Vai a Misura",
        className: "btn btn-large",
      })
    )
    cta.appendChild(
      button("2. Ottimizza", () => handleNavigate("actions"), {
        ariaLabel: "Vai a Ottimizza",
        className: "btn btn-large btn-secondary",
      })
    )
    cta.appendChild(
      button("3. Verifica", () => handleNavigate("benchmark"), {
        ariaLabel: "Vai a Verifica",
        className: "btn btn-large btn-secondary",
      })
    )
    side.appendChild(cta)
    dash.appendChild(side)

    results.replaceChildren()
    const grid = el("div", "stat-grid")
    const topHog = overview.hogs?.hogs?.[0]
    grid.appendChild(
      statTile(
        "Processo piu pesante",
        topHog ? `${topHog.cpu_percent.toFixed(0)}% CPU` : "n/d",
        topHog ? topHog.name : "Nessun hog rilevato"
      )
    )
    grid.appendChild(
      statTile(
        "Temp CPU",
        overview.thermal?.cpu_temp_celsius != null
          ? `${overview.thermal.cpu_temp_celsius} C`
          : "n/d",
        `Livello: ${overview.thermal?.level || "n/d"}`
      )
    )
    grid.appendChild(
      statTile(
        "Pulizia disco",
        formatBytes(overview.cleanup?.total_bytes),
        "Anteprima, nessuna cancellazione automatica"
      )
    )
    const enabledStartup = (overview.startup?.rows || []).filter((r) => r.enabled).length
    grid.appendChild(
      statTile(
        "Avvio",
        `${enabledStartup} attivi`,
        `${overview.startup?.rows?.length || 0} voci elencate`
      )
    )
    if (lastMeasure) {
      grid.appendChild(
        statTile("Ultima misura FPS", formatMetric(lastMeasure.session.frame.avg_fps))
      )
      grid.appendChild(
        statTile("Ultima misura CPU", formatMetric(lastMeasure.session.hardware.cpu_usage_percent))
      )
    }
    results.appendChild(grid)
    results.appendChild(el("p", "note", overview.local_measurement_note))
    status.textContent =
      score == null
        ? "Panoramica pronta. Fai il passo 1: Misura."
        : `Salute sistema ${score}%. Prossimo passo: Ottimizza, poi Verifica.`
  } catch (error) {
    results.replaceChildren()
    paintHomeShell(
      dash,
      "Panoramica lenta o non disponibile. Parti da Misura -- lo score arrivera dopo."
    )
    status.textContent = `Panoramica saltata: ${error}. L'app resta usabile.`
    results.appendChild(
      el(
        "p",
        "note",
        "Suggerimento: apri Quotidiano dopo qualche secondo, oppure vai subito a Misura."
      )
    )
  }
}

const renderMeasure = (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Avvia una sessione locale. I sensori assenti restano n/d. Poi passa a Ottimizza."
    )
  )

  const processInput = el("input")
  processInput.type = "text"
  processInput.id = "measure-process"
  processInput.setAttribute("aria-label", "Nome processo target")
  processInput.placeholder = "es. gioco.exe (opzionale)"

  const hostInput = el("input")
  hostInput.type = "text"
  hostInput.id = "measure-host"
  hostInput.setAttribute("aria-label", "Host rete per latenza")
  hostInput.placeholder = "host ping opzionale"

  const form = el("div", "panel")
  form.appendChild(field("Processo target", processInput))
  form.appendChild(field("Host rete", hostInput))

  const advanced = el("details", "advanced")
  advanced.appendChild(el("summary", null, "Opzioni avanzate (PresentMon / MangoHud)"))
  const csvInput = el("textarea")
  csvInput.id = "measure-presentmon"
  csvInput.rows = 3
  csvInput.setAttribute("aria-label", "CSV PresentMon")
  csvInput.placeholder = "CSV PresentMon opzionale"
  const mangoInput = el("textarea")
  mangoInput.id = "measure-mangohud"
  mangoInput.rows = 3
  mangoInput.setAttribute("aria-label", "Log MangoHud")
  mangoInput.placeholder = "Log MangoHud opzionale"
  advanced.appendChild(field("CSV PresentMon", csvInput))
  advanced.appendChild(field("Log MangoHud", mangoInput))
  form.appendChild(advanced)

  const status = el("p", "status", "")
  status.id = "measure-status"
  status.setAttribute("role", "status")
  const results = el("div", "results")
  results.id = "measure-results"

  const handleRun = async () => {
    status.textContent = "Misurazione in corso..."
    results.replaceChildren()
    try {
      lastMeasure = await invoke("measure_run", {
        request: {
          target_process: processInput.value.trim() || null,
          network_host: hostInput.value.trim() || null,
          presentmon_csv: csvInput.value.trim() || null,
          mangohud_log: mangoInput.value.trim() || null,
          notes: null,
        },
      })
      status.textContent = `Sessione ${lastMeasure.session.meta.session_id}. Probe: ${lastMeasure.frame_probe.status}`
      results.appendChild(renderMeasureCards(lastMeasure.session))
      const next = el("div", "cta-row")
      next.appendChild(
        button("Continua: Ottimizza", () => handleNavigate("actions"), {
          ariaLabel: "Vai a Ottimizza",
          className: "btn btn-large",
        })
      )
      results.appendChild(next)
      results.appendChild(el("p", "note", lastMeasure.local_measurement_note))
      const details = el("details", "advanced")
      details.appendChild(el("summary", null, "Tabella completa + export"))
      details.appendChild(renderMetricTable(lastMeasure.session))
      const exports = el("div", "actions-row")
      exports.appendChild(
        button("Esporta CSV", () => {
          downloadText(
            `${lastMeasure.session.meta.session_id}.csv`,
            lastMeasure.csv,
            "text/csv"
          )
        }, { ariaLabel: "Esporta CSV" })
      )
      exports.appendChild(
        button("Esporta JSON", () => {
          downloadText(
            `${lastMeasure.session.meta.session_id}.json`,
            lastMeasure.json,
            "application/json"
          )
        }, { ariaLabel: "Esporta JSON", className: "btn btn-secondary" })
      )
      details.appendChild(exports)
      results.appendChild(details)
    } catch (error) {
      status.textContent = `Misura fallita: ${error}`
    }
  }

  form.appendChild(
    button("Avvia misura", handleRun, {
      ariaLabel: "Avvia misurazione",
      className: "btn btn-large",
    })
  )
  root.appendChild(form)
  root.appendChild(status)
  root.appendChild(results)

  if (lastMeasure) {
    status.textContent = `Ultima sessione ${lastMeasure.session.meta.session_id}`
    results.appendChild(renderMeasureCards(lastMeasure.session))
  }
}

const renderBenchCard = (title, result) => {
  const card = el("div", "panel")
  card.appendChild(el("h2", null, title))
  if (!result) {
    card.appendChild(el("p", "muted", "Non ancora eseguito."))
    return card
  }
  card.appendChild(el("p", null, `Sorgente: ${result.source}`))
  card.appendChild(el("p", null, `FPS medio ${formatSpread(result.avg_fps)}`))
  card.appendChild(el("p", null, `1% low ${formatSpread(result.low_1pct_fps)}`))
  card.appendChild(el("p", null, `0.1% low ${formatSpread(result.low_0_1pct_fps)}`))
  card.appendChild(el("p", null, `Frame time ${formatSpread(result.avg_frame_time_ms)}`))
  return card
}

const parseWindowsJson = (text) => {
  const parsed = JSON.parse(text)
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error("windows deve essere un array JSON non vuoto di array di frame-time")
  }
  return parsed
}

const gainPercent = (beforeMedian, afterMedian) => {
  if (!beforeMedian || beforeMedian === 0) {
    return null
  }
  return ((afterMedian - beforeMedian) / beforeMedian) * 100
}

const renderBenchmark = async (root) => {
  const defaults = await invoke("benchmark_default_config")
  const noLocal = await invoke("benchmark_no_local_note")

  root.appendChild(
    el(
      "p",
      "lede",
      "Wizard before/after. Un cambiamento e utile solo se il guadagno supera la banda di rumore misurata."
    )
  )
  root.appendChild(el("p", "note", noLocal))

  const wizard = el("div", "wizard")
  const stepBefore = el("div", "wizard__step", "1 Before")
  const stepAfter = el("div", "wizard__step", "2 After")
  const stepCompare = el("div", "wizard__step", "3 Confronta")
  wizard.appendChild(stepBefore)
  wizard.appendChild(stepAfter)
  wizard.appendChild(stepCompare)
  root.appendChild(wizard)

  const warmup = el("input")
  warmup.type = "number"
  warmup.min = "1"
  warmup.value = String(defaults.warmup_ms)
  warmup.setAttribute("aria-label", "Warm-up millisecondi")

  const measureMs = el("input")
  measureMs.type = "number"
  measureMs.min = "1"
  measureMs.value = String(defaults.measure_ms)
  measureMs.setAttribute("aria-label", "Misura millisecondi per pass")

  const passes = el("input")
  passes.type = "number"
  passes.min = "3"
  passes.value = String(defaults.pass_count)
  passes.setAttribute("aria-label", "Numero pass")

  const windowsInput = el("textarea")
  windowsInput.rows = 4
  windowsInput.setAttribute("aria-label", "Frame time windows JSON")
  windowsInput.placeholder =
    "JSON: [[warmup ms...],[pass1...],[pass2...],[pass3...]]"

  const form = el("div", "panel")
  const advanced = el("details", "advanced")
  advanced.appendChild(el("summary", null, "Parametri e finestre frame (avanzato)"))
  advanced.appendChild(field("Warm-up (ms)", warmup))
  advanced.appendChild(field("Misura per pass (ms)", measureMs))
  advanced.appendChild(field("Pass (>=3)", passes))
  advanced.appendChild(field("Finestre frame (JSON)", windowsInput))
  form.appendChild(advanced)

  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const syncWizard = () => {
    stepBefore.classList.toggle("is-done", !!beforeBench)
    stepBefore.classList.toggle("is-current", !beforeBench)
    stepAfter.classList.toggle("is-done", !!afterBench)
    stepAfter.classList.toggle("is-current", !!beforeBench && !afterBench)
    stepCompare.classList.toggle("is-done", !!lastVerdict)
    stepCompare.classList.toggle("is-current", !!beforeBench && !!afterBench && !lastVerdict)
  }

  const refreshResults = () => {
    results.replaceChildren()
    syncWizard()
    results.appendChild(renderBenchCard("Before", beforeBench))
    results.appendChild(renderBenchCard("After", afterBench))
    if (lastVerdict) {
      const v = el("div", "panel verdict")
      v.appendChild(el("h2", null, "Verdetto"))
      const pct = gainPercent(lastVerdict.before_median, lastVerdict.after_median)
      v.appendChild(
        el(
          "p",
          "verdict-gain",
          pct == null
            ? lastVerdict.message
            : `${pct >= 0 ? "+" : ""}${pct.toFixed(1)}% FPS mediano`
        )
      )
      v.appendChild(el("p", null, lastVerdict.message))
      v.appendChild(
        el(
          "p",
          null,
          `Guadagno mediano ${lastVerdict.gain.toFixed(2)} fps (rumore ${lastVerdict.noise_band.toFixed(2)}). ${lastVerdict.before_median.toFixed(2)} -> ${lastVerdict.after_median.toFixed(2)} fps.`
        )
      )
      v.appendChild(el("p", "note", lastVerdict.explanation))
      if (lastVerdict.offer_revert) {
        v.appendChild(
          el(
            "p",
            "note",
            "Il guadagno non batte il rumore: valuta Ripristina tutto se hai applicato una modifica."
          )
        )
      }
      results.appendChild(v)
    }
  }

  const readConfig = () => ({
    warmup_ms: Number(warmup.value),
    measure_ms: Number(measureMs.value),
    pass_count: Number(passes.value),
  })

  const handlePhase = async (phase) => {
    status.textContent = `Eseguo ${phase}...`
    try {
      const config = readConfig()
      const windows = parseWindowsJson(windowsInput.value)
      const result = await invoke("benchmark_run_windows", {
        config,
        windows,
        source: "supplied_windows",
      })
      if (phase === "before") {
        beforeBench = result
      } else {
        afterBench = result
      }
      lastVerdict = null
      status.textContent = `${phase} completato (${result.source}).`
      refreshResults()
    } catch (error) {
      status.textContent = `${phase} fallito: ${error}`
    }
  }

  const handleCompare = async () => {
    if (!beforeBench || !afterBench) {
      status.textContent = "Esegui before e after prima."
      return
    }
    try {
      lastVerdict = await invoke("benchmark_compare", {
        before: beforeBench,
        after: afterBench,
      })
      status.textContent = `Verdetto: ${lastVerdict.message}`
      refreshResults()
    } catch (error) {
      status.textContent = `Confronto fallito: ${error}`
    }
  }

  const handleMock = async () => {
    status.textContent = "Coppia mock etichettata (non e una cattura live)..."
    try {
      const pair = await invoke("benchmark_run_labeled_mock", {
        beforeFrameMs: 1000 / 60,
        afterFrameMs: 1000 / 60,
      })
      beforeBench = pair[0]
      afterBench = pair[1]
      lastVerdict = pair[2]
      status.textContent =
        "Mock etichettato completato. Sorgente labeled_mock -- non e una cattura live."
      refreshResults()
    } catch (error) {
      status.textContent = `Mock fallito: ${error}`
    }
  }

  const actions = el("div", "actions-row")
  actions.appendChild(
    button("1. Esegui before", () => handlePhase("before"), {
      ariaLabel: "Esegui before",
      className: "btn btn-large",
    })
  )
  actions.appendChild(
    button("2. Esegui after", () => handlePhase("after"), {
      ariaLabel: "Esegui after",
      className: "btn btn-large",
    })
  )
  actions.appendChild(
    button("3. Confronta", handleCompare, {
      ariaLabel: "Confronta before e after",
      className: "btn btn-large",
    })
  )
  actions.appendChild(
    button("Mock etichettato", handleMock, {
      ariaLabel: "Esegui mock etichettato",
      className: "btn btn-secondary",
    })
  )
  form.appendChild(actions)
  root.appendChild(form)
  root.appendChild(status)
  root.appendChild(results)
  refreshResults()
}

const renderExplanation = (parent, explanation) => {
  const details = el("details", "advanced")
  details.appendChild(el("summary", null, "Cosa fa / rischio / come annullare"))
  const dl = el("dl", "explanation")
  const rows = [
    ["Cosa", explanation.what],
    ["Effetto atteso", explanation.expected_effect],
    ["Rischio", explanation.risk],
    ["Come annullare", explanation.how_to_undo],
    ["Misura locale", explanation.local_measurement],
  ]
  for (const [label, value] of rows) {
    dl.appendChild(el("dt", null, label))
    dl.appendChild(el("dd", null, value))
  }
  details.appendChild(dl)
  parent.appendChild(details)
}

const actionPriority = (action) => {
  let score = 40
  const state = (action.current_state || "").toLowerCase()
  if (state.includes("off") || state.includes("disabled") || state.includes("balanced")) {
    score += 25
  }
  if (state.includes("on") || state.includes("enabled") || state.includes("performance")) {
    score -= 10
  }
  if (action.kind === "destructive") {
    score += 5
  }
  if (action.kind === "report") {
    score -= 5
  }
  if (action.requires_elevation) {
    score -= 8
  }
  const risk = (action.explanation?.risk || "").toLowerCase()
  if (risk.includes("low") || risk.includes("basso")) {
    score += 15
  }
  if (risk.includes("high") || risk.includes("alto")) {
    score -= 20
  }
  return score
}

const impactBadge = (action) => {
  const p = actionPriority(action)
  if (p >= 70) {
    return { text: "Priorita alta", className: "badge" }
  }
  if (p >= 50) {
    return { text: "Priorita media", className: "badge badge-warn" }
  }
  return { text: "Priorita bassa", className: "badge badge-muted" }
}

const renderActions = async (root) => {
  const catalog = await invoke("actions_catalog")
  root.appendChild(
    el(
      "p",
      "lede",
      `Azioni per questo host (${catalog.host_os}). Ordinate per priorita. Ogni modifica e spiegata e reversibile. Poi Verifica il guadagno.`
    )
  )
  root.appendChild(el("p", "note", catalog.local_measurement_note))

  if (catalog.macos_honesty) {
    const honesty = el("div", "panel")
    honesty.appendChild(el("h2", null, "Onesta macOS"))
    honesty.appendChild(el("p", null, catalog.macos_honesty.banner))
    root.appendChild(honesty)
  }

  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const hostActions = catalog.actions
    .filter((a) => a.host_status === "available")
    .slice()
    .sort((a, b) => actionPriority(b) - actionPriority(a))
  const otherActions = catalog.actions.filter((a) => a.host_status !== "available")

  const renderCard = (action, rank) => {
    const panel = el("div", "panel action-card")
    if (rank != null) {
      panel.appendChild(el("p", "priority", `#${rank}`))
    }
    const titleRow = el("div", "cta-row")
    titleRow.appendChild(el("h2", null, action.title))
    const badge = impactBadge(action)
    titleRow.appendChild(el("span", badge.className, badge.text))
    panel.appendChild(titleRow)
    panel.appendChild(
      el(
        "p",
        "muted",
        `${action.id} · ${action.os} · ${action.kind}${
          action.requires_elevation ? " · richiede elevazione" : ""
        }`
      )
    )
    panel.appendChild(
      el("p", null, action.explanation?.expected_effect || "Effetto: vedi dettagli.")
    )
    if (action.current_state) {
      panel.appendChild(el("p", null, `Stato attuale: ${action.current_state}`))
    }
    renderExplanation(panel, action.explanation)

    if (action.host_status !== "available") {
      panel.appendChild(el("p", "note", `Non disponibile qui (${action.host_status}).`))
      return panel
    }

    const input = el("input")
    input.type = "text"
    input.setAttribute("aria-label", action.input_hint || `Input per ${action.title}`)
    input.placeholder = action.input_hint || "input opzionale"
    input.autocomplete = "off"
    if (action.kind === "needs_input" || action.kind === "toggle" || action.kind === "destructive") {
      if (action.input_hint) {
        panel.appendChild(field(action.input_hint, input))
      }
    }

    const elevCheck = el("input")
    elevCheck.type = "checkbox"
    elevCheck.setAttribute("aria-label", `Conferma elevazione per ${action.title}`)
    const elevLabel = el("label", "check")
    elevLabel.appendChild(elevCheck)
    elevLabel.appendChild(
      document.createTextNode(
        action.elevation_reason ||
          "Confermo un prompt di elevazione per questa sola azione."
      )
    )
    if (action.requires_elevation) {
      panel.appendChild(elevLabel)
    }

    const destCheck = el("input")
    destCheck.type = "checkbox"
    destCheck.setAttribute("aria-label", `Conferma cleanup per ${action.title}`)
    const destLabel = el("label", "check")
    destLabel.appendChild(destCheck)
    destLabel.appendChild(
      document.createTextNode("Confermo la pulizia dopo aver visto l'anteprima.")
    )
    if (action.kind === "destructive") {
      panel.appendChild(destLabel)
    }

    const row = el("div", "actions-row")
    if (action.exposes_switch || action.kind === "report" || action.kind === "destructive") {
      row.appendChild(
        button(
          action.kind === "report" ? "Aggiorna report" : "Applica",
          async () => {
            status.textContent = `Applico ${action.id}…`
            try {
              const outcome = await invoke("actions_apply", {
                request: {
                  id: action.id,
                  confirm_elevation: !!elevCheck.checked,
                  confirm_destructive: !!destCheck.checked,
                  input: input.value.trim() || null,
                },
              })
              if (outcome.status === "elevation_required") {
                status.textContent = outcome.message
              } else if (outcome.status === "confirm_destructive") {
                status.textContent = outcome.message
              } else if (outcome.status === "applied") {
                status.textContent = `Applicato ${outcome.action}: ${outcome.before} → ${outcome.after}`
              } else {
                status.textContent = `Fallito ${outcome.action}: ${outcome.detail}`
              }
            } catch (error) {
              status.textContent = `Apply fallito: ${error}`
            }
          },
          { ariaLabel: `Applica ${action.title}`, className: "btn btn-large" }
        )
      )
    }
    row.appendChild(
      button("Poi: Verifica", () => handleNavigate("benchmark"), {
        ariaLabel: "Vai a Verifica",
        className: "btn btn-secondary",
      })
    )
    panel.appendChild(row)
    return panel
  }

  root.appendChild(el("h2", null, "Consigliate su questo PC"))
  if (hostActions.length === 0) {
    root.appendChild(el("p", "muted", "Nessuna azione registrata per questo OS."))
  }
  hostActions.forEach((action, index) => {
    root.appendChild(renderCard(action, index + 1))
  })

  const other = el("details", "other-os")
  other.appendChild(el("summary", null, `Altri OS (${otherActions.length})`))
  for (const action of otherActions) {
    other.appendChild(renderCard(action, null))
  }
  root.appendChild(other)
  root.appendChild(status)
}

const renderProfiles = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Profili per gioco: applicano un set di azioni all'avvio e ripristinano tutto all'uscita."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const refresh = async () => {
    results.replaceChildren()
    try {
      profilesState = await invoke("profiles_get_overview")
      results.appendChild(el("p", "note", profilesState.note))

      const activePanel = el("div", "panel")
      activePanel.appendChild(el("h2", null, "Sessione attiva"))
      if (profilesState.active.active) {
        activePanel.appendChild(
          el(
            "p",
            null,
            `${profilesState.active.name} (${profilesState.active.game_key}) · fase ${profilesState.active.phase}`
          )
        )
        activePanel.appendChild(
          el("p", "muted", `Azioni: ${profilesState.active.action_ids.join(", ") || "(nessuna)"}`)
        )
        activePanel.appendChild(
          button(
            "Ripristina all'uscita",
            async () => {
              status.textContent = "Ripristino profilo…"
              try {
                const outcome = await invoke("profiles_restore", { labeledMock: false })
                status.textContent =
                  outcome.status === "restored"
                    ? `Ripristinato ${outcome.name}.`
                    : `Ripristino fallito: ${outcome.detail}`
                await refresh()
              } catch (error) {
                status.textContent = `Ripristino fallito: ${error}`
              }
            },
            { ariaLabel: "Ripristina profilo attivo" }
          )
        )
      } else {
        activePanel.appendChild(el("p", "muted", "Nessuna sessione profilo attiva."))
      }
      results.appendChild(activePanel)

      const manualPanel = el("div", "panel")
      manualPanel.appendChild(el("h2", null, "Aggiungi percorso manuale"))
      const nameInput = el("input")
      nameInput.type = "text"
      nameInput.setAttribute("aria-label", "Nome gioco")
      const pathInput = el("input")
      pathInput.type = "text"
      pathInput.setAttribute("aria-label", "Percorso installazione")
      manualPanel.appendChild(field("Nome gioco", nameInput))
      manualPanel.appendChild(field("Percorso", pathInput))
      manualPanel.appendChild(
        button(
          "Aggiungi",
          async () => {
            try {
              profilesState = await invoke("profiles_add_manual", {
                request: {
                  name: nameInput.value,
                  install_path: pathInput.value,
                },
              })
              status.textContent = "Percorso aggiunto."
              nameInput.value = ""
              pathInput.value = ""
              await refresh()
            } catch (error) {
              status.textContent = `Errore: ${error}`
            }
          },
          { ariaLabel: "Aggiungi percorso manuale" }
        )
      )
      results.appendChild(manualPanel)

      results.appendChild(el("h2", null, `Giochi rilevati (${profilesState.games.length})`))
      if (!profilesState.games.length) {
        results.appendChild(
          el("p", "muted", "Nessun gioco trovato. Aggiungi un percorso o installa un launcher supportato.")
        )
      }
      for (const game of profilesState.games) {
        const panel = el("div", "panel")
        panel.appendChild(el("h2", null, game.name))
        panel.appendChild(
          el("p", "muted", `${game.source} · ${game.game_key} · ${game.install_path}`)
        )
        const existing =
          profilesState.profiles.find((p) => p.game_key === game.game_key) || {
            game_key: game.game_key,
            name: game.name,
            action_ids: [],
          }
        const actionsInput = el("input")
        actionsInput.type = "text"
        actionsInput.value = existing.action_ids.join(", ")
        actionsInput.setAttribute("aria-label", `ID azioni per ${game.name}`)
        actionsInput.placeholder = "id azioni, separati da virgola"
        panel.appendChild(field("Azioni profilo", actionsInput))
        const row = el("div", "actions-row")
        row.appendChild(
          button(
            "Salva profilo",
            async () => {
              try {
                const actionIds = actionsInput.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean)
                await invoke("profiles_save", {
                  request: {
                    game_key: game.game_key,
                    name: game.name,
                    action_ids: actionIds,
                  },
                })
                status.textContent = `Profilo salvato per ${game.name}.`
                await refresh()
              } catch (error) {
                status.textContent = `Salvataggio fallito: ${error}`
              }
            },
            { ariaLabel: `Salva profilo ${game.name}`, className: "btn btn-secondary" }
          )
        )
        row.appendChild(
          button(
            "Applica all'avvio",
            async () => {
              try {
                const outcome = await invoke("profiles_apply", {
                  request: { game_key: game.game_key, labeled_mock: false },
                })
                status.textContent =
                  outcome.status === "applied"
                    ? `Applicato ${outcome.name}`
                    : `Fallito: ${outcome.detail}`
                await refresh()
              } catch (error) {
                status.textContent = `Apply fallito: ${error}`
              }
            },
            { ariaLabel: `Applica profilo ${game.name}` }
          )
        )
        panel.appendChild(row)
        results.appendChild(panel)
      }
      status.textContent = `${profilesState.games.length} giochi, ${profilesState.profiles.length} profili.`
    } catch (error) {
      status.textContent = `Profili falliti: ${error}`
    }
  }

  root.appendChild(
    button("Aggiorna giochi e profili", refresh, { ariaLabel: "Aggiorna profili" })
  )
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderChangelog = async (root) => {
  root.appendChild(
    el("p", "lede", "Cronologia locale: tempo, azione, before/after, stato ripristino.")
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const refresh = async () => {
    results.replaceChildren()
    try {
      const rows = await invoke("actions_change_log")
      if (!rows.length) {
        results.appendChild(el("p", "muted", "Nessuna modifica registrata."))
        status.textContent = "Cronologia vuota."
        return
      }
      const table = el("table", "metrics")
      table.setAttribute("aria-label", "Cronologia modifiche")
      const head = document.createElement("thead")
      const hr = document.createElement("tr")
      for (const label of ["ID", "Tempo", "Azione", "Before", "After", "Stato", ""]) {
        hr.appendChild(el("th", null, label))
      }
      head.appendChild(hr)
      table.appendChild(head)
      const body = document.createElement("tbody")
      for (const row of rows) {
        const tr = document.createElement("tr")
        tr.appendChild(el("td", null, String(row.id)))
        tr.appendChild(el("td", null, String(row.time_unix_ms)))
        tr.appendChild(el("td", null, row.action))
        tr.appendChild(el("td", "break", row.before))
        tr.appendChild(el("td", "break", row.after))
        tr.appendChild(el("td", null, row.restore_status))
        const cell = document.createElement("td")
        if (row.restore_status === "outstanding") {
          cell.appendChild(
            button(
              "Ripristina",
              async () => {
                try {
                  const updated = await invoke("actions_restore_entry", { entryId: row.id })
                  status.textContent = `Ripristinato ${updated.id}.`
                  await refresh()
                } catch (error) {
                  status.textContent = `Ripristino fallito: ${error}`
                }
              },
              { ariaLabel: `Ripristina ${row.id}`, className: "btn btn-secondary" }
            )
          )
        }
        tr.appendChild(cell)
        body.appendChild(tr)
      }
      table.appendChild(body)
      results.appendChild(table)
      status.textContent = `${rows.length} voci.`
    } catch (error) {
      status.textContent = `Cronologia fallita: ${error}`
    }
  }

  root.appendChild(button("Aggiorna", refresh, { ariaLabel: "Aggiorna cronologia" }))
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderRestoreAll = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Ripristina tutte le modifiche Hertzline ancora aperte. Stesso percorso usato da crash recovery e disinstallazione."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  root.appendChild(
    button(
      "Ripristina tutto",
      async () => {
        status.textContent = "Ripristino in corso…"
        results.replaceChildren()
        try {
          const report = await invoke("actions_restore_all")
          status.textContent = `Ripristino ${report.status} (ok=${report.ok}).`
          const list = el("ul", "cap-list")
          for (const item of report.items) {
            list.appendChild(el("li", null, item))
          }
          results.appendChild(list)
        } catch (error) {
          status.textContent = `Ripristino fallito: ${error}`
        }
      },
      { ariaLabel: "Ripristina tutte le modifiche", className: "btn btn-large" }
    )
  )
  root.appendChild(status)
  root.appendChild(results)
}

const renderEveryday = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Modalita quotidiana: avvio, processi pesanti, alimentazione, pulizia disco, termiche, report settimanale."
    )
  )
  const status = el("p", "status", "Carico...")
  const results = el("div", "results")
  root.appendChild(status)
  root.appendChild(results)

  const refresh = async () => {
    status.textContent = "Aggiorno..."
    results.replaceChildren()
    try {
      const overview = await withTimeout(
        invoke("everyday_get_overview"),
        8000,
        "everyday_get_overview"
      )
      homeCache = overview
      const { score } = scoreFromOverview(overview, lastMeasure)
      results.appendChild(
        statTile("Salute stimata", score == null ? "n/d" : `${score}%`, "Da indicatori reali")
      )
      results.appendChild(el("p", "note", overview.local_measurement_note))

      results.appendChild(el("h2", null, "Impatto avvio"))
      const list = el("ul", "list")
      for (const row of overview.startup.rows) {
        list.appendChild(
          el(
            "li",
            null,
            `${row.name} · ${row.enabled ? "attivo" : "disattivo"} · ${row.impact_summary}`
          )
        )
      }
      if (!overview.startup.rows.length) {
        results.appendChild(el("p", "muted", "Nessuna voce avvio."))
      } else {
        results.appendChild(list)
      }

      results.appendChild(el("h2", null, "Processi pesanti"))
      const hogList = el("ul", "list")
      for (const hog of overview.hogs.hogs) {
        hogList.appendChild(
          el(
            "li",
            null,
            `${hog.name} · CPU ${hog.cpu_percent.toFixed(1)}% · RAM ${formatBytes(hog.ram_bytes)}`
          )
        )
      }
      results.appendChild(hogList)

      results.appendChild(el("h2", null, "Profilo alimentazione"))
      results.appendChild(
        el(
          "p",
          null,
          overview.power.available
            ? `Attuale: ${overview.power.preset || "sconosciuto"}`
            : overview.power.message
        )
      )
      if (overview.power.available) {
        for (const [id, label] of [
          ["battery_saver", "Risparmio"],
          ["balanced", "Bilanciato"],
          ["performance", "Prestazioni"],
        ]) {
          results.appendChild(
            button(
              `Applica ${label}`,
              async () => {
                const outcome = await invoke("everyday_apply_preset", {
                  request: { preset: id },
                })
                status.textContent =
                  outcome.status === "applied"
                    ? `Preset applicato (${outcome.entry_id}).`
                    : `Fallito: ${outcome.detail}`
                await refresh()
              },
              { ariaLabel: `Applica ${label}` }
            )
          )
        }
      }

      results.appendChild(el("h2", null, "Pulizia disco (anteprima)"))
      results.appendChild(
        el("p", null, `Totale anteprima: ${formatBytes(overview.cleanup.total_bytes)}`)
      )
      results.appendChild(
        button(
          "Elimina file in anteprima",
          async () => {
            const outcome = await invoke("everyday_apply_cleanup", {
              request: { confirm: true },
            })
            status.textContent =
              outcome.status === "applied"
                ? `Rimossi ${formatBytes(outcome.bytes_removed)}.`
                : outcome.message || outcome.detail
            await refresh()
          },
          { ariaLabel: "Conferma pulizia disco" }
        )
      )

      results.appendChild(el("h2", null, "Termiche"))
      results.appendChild(
        el(
          "p",
          null,
          `Livello ${overview.thermal.level}. CPU ${overview.thermal.cpu_temp_celsius ?? "n/d"} / GPU ${overview.thermal.gpu_temp_celsius ?? "n/d"} C`
        )
      )
      status.textContent = "Panoramica quotidiana pronta."
    } catch (error) {
      status.textContent = `Errore: ${error}`
    }
  }

  root.appendChild(
    button("Aggiorna", refresh, { ariaLabel: "Aggiorna quotidiano", className: "btn btn-large" })
  )
  await refresh()
}

const renderOverlay = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Overlay FPS/frame-time opzionale. Mai injection nel gioco. Off di default con anti-cheat."
    )
  )
  const status = el("p", "status", "Carico...")
  const results = el("div", "results")
  root.appendChild(status)
  root.appendChild(results)

  const pathInput = document.createElement("input")
  pathInput.type = "text"
  pathInput.placeholder = "Percorso installazione gioco (opzionale)"
  pathInput.setAttribute("aria-label", "Percorso installazione")

  const enabledInput = document.createElement("input")
  enabledInput.type = "checkbox"
  enabledInput.setAttribute("aria-label", "Abilita overlay")

  const acInput = document.createElement("input")
  acInput.type = "checkbox"
  acInput.setAttribute("aria-label", "Consenti con anti-cheat")

  const applyOverview = (overview) => {
    status.textContent = overview.decision_enabled
      ? `Overlay attivo (${overview.method_id})`
      : `Overlay off (${overview.block_reason || "preferenza"})`
    results.replaceChildren()
    results.appendChild(el("p", null, overview.note))
    results.appendChild(
      el(
        "p",
        null,
        `Metodo: ${overview.method_id} / ${overview.method_status}. Injection: ${overview.injects_into_game ? "si" : "no"}.`
      )
    )
    results.appendChild(
      el(
        "p",
        null,
        `Anti-cheat: ${overview.anti_cheat_detected ? "rilevato" : "non rilevato"}.`
      )
    )
    const panel = overview.panel
    results.appendChild(
      el(
        "p",
        null,
        `Pannello: ${panel.status}; FPS ${panel.avg_fps ?? "n/d"}; frame ${panel.avg_frame_time_ms ?? "n/d"} ms.`
      )
    )
    enabledInput.checked = overview.settings.user_enabled && !overview.settings.globally_disabled
    acInput.checked = overview.settings.allow_with_anti_cheat
  }

  const refresh = async () => {
    try {
      const overview = await invoke("overlay_get_overview", {
        installPath: pathInput.value.trim() || null,
      })
      applyOverview(overview)
    } catch (error) {
      status.textContent = `Errore: ${error}`
    }
  }

  root.appendChild(field("Percorso installazione", pathInput))
  const enabledLabel = el("label", "field")
  enabledLabel.appendChild(enabledInput)
  enabledLabel.appendChild(el("span", "field-label", " Abilita overlay (se permesso)"))
  root.appendChild(enabledLabel)
  const acLabel = el("label", "field")
  acLabel.appendChild(acInput)
  acLabel.appendChild(el("span", "field-label", " Consenti con anti-cheat (solo metodi documentati)"))
  root.appendChild(acLabel)

  const row = el("div", "actions")
  row.appendChild(
    button(
      "Salva",
      async () => {
        try {
          const overview = await invoke("overlay_save_settings", {
            request: {
              globally_disabled: false,
              user_enabled: enabledInput.checked,
              allow_with_anti_cheat: acInput.checked,
              install_path: pathInput.value.trim() || null,
            },
          })
          applyOverview(overview)
        } catch (error) {
          status.textContent = `Salvataggio fallito: ${error}`
        }
      },
      { ariaLabel: "Salva overlay" }
    )
  )
  row.appendChild(
    button(
      "Disattiva globalmente",
      async () => {
        try {
          applyOverview(await invoke("overlay_disable_globally"))
        } catch (error) {
          status.textContent = `Errore: ${error}`
        }
      },
      { ariaLabel: "Disattiva overlay globalmente", className: "btn btn-secondary" }
    )
  )
  row.appendChild(button("Aggiorna", refresh, { ariaLabel: "Aggiorna overlay" }))
  root.appendChild(row)
  await refresh()
}

const renderRoute = async (routes, info) => {
  const id = parseRouteId()
  const route = routes.find((item) => item.id === id) || routes[0]
  const activeId = route.id
  if (window.location.hash !== route.hash_path) {
    window.location.hash = route.hash_path
  }
  renderNav(routes, activeId)
  updateStepStrip(activeId)
  document.querySelector("#title").textContent = route.title
  const body = document.querySelector("#body")
  body.replaceChildren()
  if (activeId === "home") {
    await renderHome(body)
  } else if (activeId === "measure") {
    renderMeasure(body)
  } else if (activeId === "benchmark") {
    await renderBenchmark(body)
  } else if (activeId === "actions") {
    await renderActions(body)
  } else if (activeId === "profiles") {
    await renderProfiles(body)
  } else if (activeId === "everyday") {
    await renderEveryday(body)
  } else if (activeId === "overlay") {
    await renderOverlay(body)
  } else if (activeId === "changelog") {
    await renderChangelog(body)
  } else if (activeId === "restore-all") {
    await renderRestoreAll(body)
  } else {
    body.appendChild(el("p", "lede", "Route registrata."))
  }
  document.querySelector("#meta").textContent =
    `Shell non privilegiata v${info.app_version} (core ${info.core_version}). Nessuna elevazione all'avvio.`
  document.querySelector("#main").focus({ preventScroll: true })
}

const handleNavigate = async (id) => {
  const hashPath = await invoke("open_route", { id })
  if (window.location.hash !== hashPath) {
    window.location.hash = hashPath
  }
  await renderRoute(shellRoutes, shellInfo)
}

window.addEventListener("hashchange", () => {
  if (!shellInfo) {
    return
  }
  renderRoute(shellRoutes, shellInfo)
})

window.addEventListener("DOMContentLoaded", async () => {
  shellInfo = await invoke("get_shell_info")
  shellRoutes = shellInfo.routes
  if (!window.location.hash) {
    window.location.hash = shellRoutes[0].hash_path
  }
  await renderRoute(shellRoutes, shellInfo)
})
