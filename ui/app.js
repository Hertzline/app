const { invoke } = window.__TAURI__.core

let shellRoutes = []
let shellInfo = null
let lastMeasure = null
let beforeBench = null
let afterBench = null
let lastVerdict = null
let profilesState = null
let homeCache = null
let renderToken = 0
let ignoreHashChange = false

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
    return "n/a"
  }
  if (metric.status === "available") {
    const unit = metric.unit ? ` ${metric.unit}` : ""
    return `${metric.value}${unit}`
  }
  return "n/a"
}

const formatTextMetric = (metric) => {
  if (!metric || metric.status !== "available") {
    return "n/a"
  }
  return metric.value
}

const formatSpread = (spread) => {
  if (!spread) {
    return "n/a"
  }
  return `mediana ${spread.median.toFixed(1)} (IQR ${spread.iqr.toFixed(1)}; ${spread.min.toFixed(1)}--${spread.max.toFixed(1)})`
}

const formatBytes = (value) => {
  if (value === undefined || value === null) {
    return "n/a"
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
  if (thermal?.level === "warning" || thermal?.level === "high" || thermal?.level === "critical") {
    parts.push(40)
  } else if (thermal?.level === "ok" || thermal?.level === "normal") {
    parts.push(88)
  }
  const preset = (overview?.power?.preset || "").toLowerCase()
  if (preset.includes("high") || preset.includes("performance") || preset.includes("prestaz")) {
    parts.push(92)
  } else if (preset.includes("balanced") || preset.includes("bilanc")) {
    parts.push(72)
  } else if (preset.includes("saver") || preset.includes("risparmio") || preset.includes("power")) {
    parts.push(48)
  } else if (overview?.power?.available) {
    parts.push(65)
  }
  const startupOn = (overview?.startup?.rows || []).filter((r) => r.enabled).length
  if (overview?.startup?.rows?.length) {
    parts.push(Math.max(30, 100 - startupOn * 6))
  }
  const cleanup = overview?.cleanup?.total_bytes
  if (typeof cleanup === "number" && cleanup > 0) {
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

const paintHomeDash = (dash, score, message) => {
  dash.replaceChildren()
  const ring = el("div", "score-ring")
  ring.style.setProperty("--pct", score == null ? 0 : score)
  const inner = el("div", "score-ring__inner")
  inner.appendChild(el("span", "score-ring__value", score == null ? "--" : `${score}%`))
  inner.appendChild(el("span", "score-ring__label", "health"))
  ring.appendChild(inner)
  dash.appendChild(ring)
  const side = el("div")
  side.appendChild(el("p", null, message))
  const cta = el("div", "cta-row")
  cta.appendChild(
    button("Optimize now", () => handleNavigate("actions"), {
      ariaLabel: "Go to Optimize",
      className: "btn btn-large",
    })
  )
  cta.appendChild(
    button("Measure", () => handleNavigate("measure"), {
      ariaLabel: "Go to Measure",
      className: "btn btn-large btn-secondary",
    })
  )
  cta.appendChild(
    button("Verify", () => handleNavigate("benchmark"), {
      ariaLabel: "Go to Verify",
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
      "Start with Optimize: apply the 3 recommended tips (Game Mode, performance power plan, Game Bar). Then measure and verify the real gain."
    )
  )
  const status = el("p", "status", "Ready.")
  status.setAttribute("role", "status")
  const dash = el("div", "panel hero-dash")
  const results = el("div", "results")
  paintHomeDash(
    dash,
    null,
    "Un click su Optimize now. Le modifiche sono spiegate e reversibili."
  )
  root.appendChild(status)
  root.appendChild(dash)
  root.appendChild(results)

  try {
    const overview = await withTimeout(
      invoke("everyday_get_overview_lite"),
      2500,
      "everyday_get_overview_lite"
    )
    homeCache = overview
    const { score, samples } = scoreFromOverview(overview, lastMeasure)
    paintHomeDash(
      dash,
      score,
      score == null
        ? "Score incomplete until you measure. You can still optimize now."
        : `Score ${score}% da ${samples} real indicators. Not an FPS claim.`
    )
    results.replaceChildren()
    const grid = el("div", "stat-grid")
    grid.appendChild(
      statTile(
        "Power plan",
        overview.power?.preset || (overview.power?.available ? "letta" : "n/a"),
        overview.power?.available ? "Current plan" : "Unread"
      )
    )
    if (lastMeasure) {
      grid.appendChild(
        statTile("Last measure FPS", formatMetric(lastMeasure.session.frame.avg_fps))
      )
      grid.appendChild(
        statTile("Last measure CPU", formatMetric(lastMeasure.session.hardware.cpu_usage_percent))
      )
    } else {
      grid.appendChild(statTile("Last measure", "none yet", "After Optimize"))
    }
    results.appendChild(grid)
    status.textContent =
      score == null
        ? "Vai su Optimize now, poi Measure e Verify."
        : `Health ${score}%. Prossimo passo: Optimize now.`
  } catch (error) {
    status.textContent = `Overview skipped (${error}). Optimize is still available.`
    results.replaceChildren()
    results.appendChild(
      el("p", "note", "Nothing blocking: open Optimize and apply the recommended pack.")
    )
  }
}

const renderMeasure = (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Run a local session. Missing sensors stay n/a. Then go to Optimize."
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
    status.textContent = "Measuring..."
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
        button("Continue: Optimize", () => handleNavigate("actions"), {
          ariaLabel: "Go to Optimize",
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
      status.textContent = `Measure failed: ${error}`
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
  measureMs.setAttribute("aria-label", "Measure millisecondi per pass")

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
  advanced.appendChild(field("Measure per pass (ms)", measureMs))
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
            "Gain did not beat noise: consider Restore all if you applied a change."
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
      status.textContent = `${phase} failed: ${error}`
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
      status.textContent = `Confronto failed: ${error}`
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
      status.textContent = `Mock failed: ${error}`
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
    ["Measure locale", explanation.local_measurement],
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

const ACTION_EN = {
  "windows.game_mode": {
    title: "Windows Game Mode",
    blurb: "Tells Windows to prioritize the game. Usually the first tip to enable.",
  },
  "windows.power_plan": {
    title: "High Performance power plan",
    blurb: "Switches to the High Performance plan. More power, more draw.",
  },
  "windows.game_bar": {
    title: "Turn off Game Bar / capture",
    blurb: "Cuts Xbox overlay and capture that can steal frames.",
  },
  "windows.hags": {
    title: "HAGS (GPU scheduling)",
    blurb: "Hardware-accelerated GPU scheduling. Needs elevation.",
  },
}

const RECOMMENDED_IDS = ["windows.game_mode", "windows.power_plan", "windows.game_bar"]

const friendlyTitle = (action) => ACTION_EN[action.id]?.title || action.title

const friendlyBlurb = (action) =>
  ACTION_EN[action.id]?.blurb || action.explanation?.expected_effect || "See details."

const stateChip = (state, actionId) => {
  if (!state) {
    return { text: "state n/a", className: "badge badge-muted" }
  }
  const raw = state.toLowerCase()
  if (actionId === "windows.game_bar") {
    if (raw.includes("off") || raw.includes("disabled") || raw.includes("false")) {
      return { text: state, className: "badge" }
    }
    return { text: state, className: "badge badge-warn" }
  }
  if (raw.includes("balanced") || raw.includes("disabled") || raw === "off" || raw.includes("false")) {
    return { text: state, className: "badge badge-warn" }
  }
  return { text: state, className: "badge" }
}

const applyOne = async (action, status, opts = {}) => {
  status.textContent = `Applying ${friendlyTitle(action)}…`
  const outcome = await invoke("actions_apply", {
    request: {
      id: action.id,
      confirm_elevation: !!opts.confirmElevation,
      confirm_destructive: !!opts.confirmDestructive,
      input: opts.input || null,
    },
  })
  if (outcome.status === "elevation_required") {
    status.textContent = `${friendlyTitle(action)}: needs elevation confirm. Check the box and retry.`
    return outcome
  }
  if (outcome.status === "confirm_destructive") {
    status.textContent = outcome.message
    return outcome
  }
  if (outcome.status === "applied") {
    status.textContent = `OK ${friendlyTitle(action)}: ${outcome.before} → ${outcome.after}`
    return outcome
  }
  status.textContent = `Failed ${friendlyTitle(action)}: ${outcome.detail || outcome.message || "?"}`
  return outcome
}

const renderActions = async (root) => {
  const catalog = await invoke("actions_catalog")
  root.appendChild(
    el(
      "p",
      "lede",
      "Three recommendations for this PC. Applying them is reversible. Then go to Verify."
    )
  )

  const status = el("p", "status", "Choose Apply all or a single action.")
  status.setAttribute("role", "status")

  const hostActions = catalog.actions.filter((a) => a.host_status === "available")
  const byId = Object.fromEntries(hostActions.map((a) => [a.id, a]))
  const recommended = RECOMMENDED_IDS.map((id) => byId[id]).filter(Boolean)
  const rest = hostActions.filter((a) => !RECOMMENDED_IDS.includes(a.id))
  const otherActions = catalog.actions.filter((a) => a.host_status !== "available")

  const pack = el("div", "panel optimize-pack")
  pack.appendChild(el("h2", null, "Recommended (1 click)"))
  pack.appendChild(
    el(
      "p",
      "muted",
      "Game Mode + performance plan + Game Bar off. No invented FPS: verify afterward with Measure/Verify."
    )
  )

  const elevAll = el("input")
  elevAll.type = "checkbox"
  elevAll.id = "elev-all"
  elevAll.setAttribute("aria-label", "Conferma elevazione se richiesta")
  const elevAllLabel = el("label", "check")
  elevAllLabel.appendChild(elevAll)
  elevAllLabel.appendChild(
    document.createTextNode("If Windows asks for admin, I confirm elevation only for these actions.")
  )
  pack.appendChild(elevAllLabel)

  const list = el("div", "rec-list")
  for (const action of recommended) {
    const row = el("div", "rec-row")
    const left = el("div")
    left.appendChild(el("p", "rec-title", friendlyTitle(action)))
    left.appendChild(el("p", "muted", friendlyBlurb(action)))
    if (action.current_state) {
      const chip = stateChip(action.current_state, action.id)
      left.appendChild(el("span", chip.className, `Now: ${chip.text}`))
    }
    row.appendChild(left)
    row.appendChild(
      button(
        "Apply",
        async () => {
          try {
            await applyOne(action, status, { confirmElevation: elevAll.checked })
          } catch (error) {
            status.textContent = `Error: ${error}`
          }
        },
        { ariaLabel: `Apply ${friendlyTitle(action)}`, className: "btn" }
      )
    )
    list.appendChild(row)
  }
  pack.appendChild(list)

  const packRow = el("div", "cta-row")
  packRow.appendChild(
    button(
      "Apply all 3",
      async () => {
        if (!recommended.length) {
          status.textContent = "No recommended actions available on this OS."
          return
        }
        status.textContent = "Applying le 3 consigliate…"
        for (const action of recommended) {
          try {
            const outcome = await applyOne(action, status, {
              confirmElevation: elevAll.checked,
            })
            if (outcome.status === "elevation_required") {
              return
            }
          } catch (error) {
            status.textContent = `Error on ${friendlyTitle(action)}: ${error}`
            return
          }
        }
        status.textContent = "Recommended pack applied. Now Measure, then Verify."
      },
      { ariaLabel: "Apply all recommended", className: "btn btn-large" }
    )
  )
  packRow.appendChild(
    button("Next: Verify gain", () => handleNavigate("benchmark"), {
      ariaLabel: "Go to Verify",
      className: "btn btn-large btn-secondary",
    })
  )
  packRow.appendChild(
    button("Measure first", () => handleNavigate("measure"), {
      ariaLabel: "Go to Measure",
      className: "btn btn-secondary",
    })
  )
  pack.appendChild(packRow)
  root.appendChild(pack)
  root.appendChild(status)

  const advanced = el("details", "advanced")
  advanced.appendChild(el("summary", null, `More actions on this PC (${rest.length})`))
  for (const action of rest) {
    const panel = el("div", "panel action-card")
    panel.appendChild(el("h2", null, friendlyTitle(action)))
    panel.appendChild(el("p", "muted", action.id))
    panel.appendChild(el("p", null, friendlyBlurb(action)))
    if (action.current_state) {
      panel.appendChild(el("p", null, `State: ${action.current_state}`))
    }
    renderExplanation(panel, action.explanation)

    const input = el("input")
    input.type = "text"
    input.setAttribute("aria-label", action.input_hint || `Input ${action.title}`)
    input.placeholder = action.input_hint || "opzionale"
    if (action.input_hint) {
      panel.appendChild(field(action.input_hint, input))
    }

    const elevCheck = el("input")
    elevCheck.type = "checkbox"
    elevCheck.setAttribute("aria-label", `Elevazione ${action.title}`)
    if (action.requires_elevation) {
      const elevLabel = el("label", "check")
      elevLabel.appendChild(elevCheck)
      elevLabel.appendChild(document.createTextNode("Confermo elevazione per questa azione"))
      panel.appendChild(elevLabel)
    }

    const destCheck = el("input")
    destCheck.type = "checkbox"
    if (action.kind === "destructive") {
      const destLabel = el("label", "check")
      destLabel.appendChild(destCheck)
      destLabel.appendChild(document.createTextNode("Confermo cleanup"))
      panel.appendChild(destLabel)
    }

    if (action.exposes_switch || action.kind === "report" || action.kind === "destructive") {
      panel.appendChild(
        button(
          action.kind === "report" ? "Refresh" : "Apply",
          async () => {
            try {
              await applyOne(action, status, {
                confirmElevation: elevCheck.checked,
                confirmDestructive: destCheck.checked,
                input: input.value.trim() || null,
              })
            } catch (error) {
              status.textContent = `Error: ${error}`
            }
          },
          { ariaLabel: `Apply ${friendlyTitle(action)}` }
        )
      )
    }
    advanced.appendChild(panel)
  }
  root.appendChild(advanced)

  if (otherActions.length) {
    const other = el("details", "other-os")
    other.appendChild(el("summary", null, `Altri OS (${otherActions.length})`))
    for (const action of otherActions) {
      const panel = el("div", "panel")
      panel.appendChild(el("h2", null, friendlyTitle(action)))
      panel.appendChild(el("p", "note", `Non disponibile qui (${action.host_status}).`))
      other.appendChild(panel)
    }
    root.appendChild(other)
  }
}

const renderProfiles = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Built-in desk modes plus per-game profiles. Modes set the OS power plan and adapt when you plug in or unplug."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const refresh = async () => {
    results.replaceChildren()
    try {
      const modes = await invoke("modes_get_overview")
      const modesPack = el("div", "panel optimize-pack")
      modesPack.appendChild(el("h2", null, "Base modes"))
      modesPack.appendChild(el("p", "muted", modes.note))
      modesPack.appendChild(
        el("p", null, `Power: ${modes.power_source.label}. ${modes.power_source.note}`)
      )
      const modeList = el("div", "rec-list")
      for (const mode of modes.modes) {
        const row = el("div", "rec-row")
        const left = el("div")
        left.appendChild(el("p", "rec-title", mode.title))
        left.appendChild(el("p", "muted", mode.blurb))
        left.appendChild(
          el(
            "p",
            "muted",
            `Plan now: ${mode.resolved_preset}${mode.adapts_to_power ? " (adapts to AC/battery)" : ""}`
          )
        )
        left.appendChild(el("p", "muted", mode.fans_note))
        if (mode.active) {
          left.appendChild(el("span", "badge", "Active"))
        }
        row.appendChild(left)
        row.appendChild(
          button(
            mode.active ? "Re-apply" : "Apply mode",
            async () => {
              status.textContent = `Applying ${mode.title}…`
              try {
                const outcome = await invoke("modes_apply", { request: { id: mode.id } })
                if (outcome.status === "applied") {
                  status.textContent = `${mode.title} applied → ${outcome.preset} (${outcome.power_source}).`
                  await refresh()
                } else {
                  status.textContent = `Failed: ${outcome.detail}`
                }
              } catch (error) {
                status.textContent = `Error: ${error}`
              }
            },
            {
              ariaLabel: `Apply ${mode.title} mode`,
              className: mode.active ? "btn btn-secondary" : "btn",
            }
          )
        )
        modeList.appendChild(row)
      }
      modesPack.appendChild(modeList)
      results.appendChild(modesPack)

      profilesState = await invoke("profiles_get_overview")
      results.appendChild(el("p", "note", profilesState.note))

      const activePanel = el("div", "panel")
      activePanel.appendChild(el("h2", null, "Active game session"))
      if (profilesState.active.active) {
        activePanel.appendChild(
          el(
            "p",
            null,
            `${profilesState.active.name} (${profilesState.active.game_key}) · phase ${profilesState.active.phase}`
          )
        )
        activePanel.appendChild(
          el("p", "muted", `Actions: ${profilesState.active.action_ids.join(", ") || "(none)"}`)
        )
        activePanel.appendChild(
          button(
            "Restore on exit",
            async () => {
              status.textContent = "Restoring profile…"
              try {
                const outcome = await invoke("profiles_restore", { labeledMock: false })
                status.textContent =
                  outcome.status === "restored"
                    ? `Restored ${outcome.name}.`
                    : `Restore failed: ${outcome.detail}`
                await refresh()
              } catch (error) {
                status.textContent = `Restore failed: ${error}`
              }
            },
            { ariaLabel: "Restore active profile" }
          )
        )
      } else {
        activePanel.appendChild(el("p", "muted", "No active game profile session."))
      }
      results.appendChild(activePanel)

      const manualPanel = el("div", "panel")
      manualPanel.appendChild(el("h2", null, "Add manual path"))
      const nameInput = el("input")
      nameInput.type = "text"
      nameInput.setAttribute("aria-label", "Game name")
      const pathInput = el("input")
      pathInput.type = "text"
      pathInput.setAttribute("aria-label", "Install path")
      manualPanel.appendChild(field("Game name", nameInput))
      manualPanel.appendChild(field("Path", pathInput))
      manualPanel.appendChild(
        button(
          "Add",
          async () => {
            try {
              profilesState = await invoke("profiles_add_manual", {
                request: {
                  name: nameInput.value,
                  install_path: pathInput.value,
                },
              })
              status.textContent = "Path added."
              nameInput.value = ""
              pathInput.value = ""
              await refresh()
            } catch (error) {
              status.textContent = `Error: ${error}`
            }
          },
          { ariaLabel: "Add manual path" }
        )
      )
      results.appendChild(manualPanel)

      results.appendChild(el("h2", null, `Detected games (${profilesState.games.length})`))
      if (!profilesState.games.length) {
        results.appendChild(
          el("p", "muted", "No games found. Add a path or install a supported launcher.")
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
        actionsInput.setAttribute("aria-label", `Action IDs for ${game.name}`)
        actionsInput.placeholder = "action ids, comma-separated"
        panel.appendChild(field("Profile actions", actionsInput))
        const row = el("div", "actions-row")
        row.appendChild(
          button(
            "Save profile",
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
                status.textContent = `Saved profile for ${game.name}.`
                await refresh()
              } catch (error) {
                status.textContent = `Save failed: ${error}`
              }
            },
            { ariaLabel: `Save profile ${game.name}`, className: "btn btn-secondary" }
          )
        )
        row.appendChild(
          button(
            "Apply on launch",
            async () => {
              try {
                const outcome = await invoke("profiles_apply", {
                  request: { game_key: game.game_key, labeled_mock: false },
                })
                status.textContent =
                  outcome.status === "applied"
                    ? `Applied ${outcome.name}`
                    : `Failed: ${outcome.detail}`
                await refresh()
              } catch (error) {
                status.textContent = `Apply failed: ${error}`
              }
            },
            { ariaLabel: `Apply profile ${game.name}` }
          )
        )
        panel.appendChild(row)
        results.appendChild(panel)
      }
      status.textContent = `${profilesState.games.length} games, ${profilesState.profiles.length} profiles.`
    } catch (error) {
      status.textContent = `Profiles failed: ${error}`
    }
  }

  root.appendChild(
    button("Refresh games and profiles", refresh, { ariaLabel: "Refresh profiles" })
  )
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderChangelog = async (root) => {
  root.appendChild(
    el("p", "lede", "Local history: time, action, before/after, restore status.")
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const refresh = async () => {
    results.replaceChildren()
    try {
      const rows = await invoke("actions_change_log")
      if (!rows.length) {
        results.appendChild(el("p", "muted", "No changes recorded."))
        status.textContent = "History empty."
        return
      }
      const table = el("table", "metrics")
      table.setAttribute("aria-label", "Change history")
      const head = document.createElement("thead")
      const hr = document.createElement("tr")
      for (const label of ["ID", "Time", "Action", "Before", "After", "Status", ""]) {
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
              "Restore",
              async () => {
                try {
                  const updated = await invoke("actions_restore_entry", { entryId: row.id })
                  status.textContent = `Restoreto ${updated.id}.`
                  await refresh()
                } catch (error) {
                  status.textContent = `Restore failed: ${error}`
                }
              },
              { ariaLabel: `Restore ${row.id}`, className: "btn btn-secondary" }
            )
          )
        }
        tr.appendChild(cell)
        body.appendChild(tr)
      }
      table.appendChild(body)
      results.appendChild(table)
      status.textContent = `${rows.length} entries.`
    } catch (error) {
      status.textContent = `History failed: ${error}`
    }
  }

  root.appendChild(button("Refresh", refresh, { ariaLabel: "Refresh history" }))
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderRestoreAll = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Restore tutte le modifiche Hertzline ancora aperte. Stesso percorso usato da crash recovery e disinstallazione."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  root.appendChild(
    button(
      "Restore tutto",
      async () => {
        status.textContent = "Restoring…"
        results.replaceChildren()
        try {
          const report = await invoke("actions_restore_all")
          status.textContent = `Restore ${report.status} (ok=${report.ok}).`
          const list = el("ul", "cap-list")
          for (const item of report.items) {
            list.appendChild(el("li", null, item))
          }
          results.appendChild(list)
        } catch (error) {
          status.textContent = `Restore failed: ${error}`
        }
      },
      { ariaLabel: "Restore tutte le modifiche", className: "btn btn-large" }
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
      "Everyday mode: startup, heavy processes, power, disk cleanup, thermal, weekly report."
    )
  )
  const status = el("p", "status", "Loading...")
  const results = el("div", "results")
  root.appendChild(status)
  root.appendChild(results)

  const refresh = async () => {
    status.textContent = "Refreshing..."
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
        statTile("Estimated health", score == null ? "n/a" : `${score}%`, "From real indicators")
      )
      results.appendChild(el("p", "note", overview.local_measurement_note))

      results.appendChild(el("h2", null, "Startup impact"))
      const list = el("ul", "list")
      for (const row of overview.startup.rows) {
        list.appendChild(
          el(
            "li",
            null,
            `${row.name} · ${row.enabled ? "enabled" : "disabled"} · ${row.impact_summary}`
          )
        )
      }
      if (!overview.startup.rows.length) {
        results.appendChild(el("p", "muted", "No startup entries."))
      } else {
        results.appendChild(list)
      }

      results.appendChild(el("h2", null, "Heavy processes"))
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

      results.appendChild(el("h2", null, "Power profile"))
      results.appendChild(
        el(
          "p",
          null,
          overview.power.available
            ? `Current: ${overview.power.preset || "unknown"}`
            : overview.power.message
        )
      )
      if (overview.power.available) {
        for (const [id, label] of [
          ["battery_saver", "Battery saver"],
          ["balanced", "Balanced"],
          ["performance", "Performance"],
        ]) {
          results.appendChild(
            button(
              `Apply ${label}`,
              async () => {
                const outcome = await invoke("everyday_apply_preset", {
                  request: { preset: id },
                })
                status.textContent =
                  outcome.status === "applied"
                    ? `Preset applied (${outcome.entry_id}).`
                    : `Fallito: ${outcome.detail}`
                await refresh()
              },
              { ariaLabel: `Apply ${label}` }
            )
          )
        }
      }

      results.appendChild(el("h2", null, "Disk cleanup (preview)"))
      results.appendChild(
        el("p", null, `Preview total: ${formatBytes(overview.cleanup.total_bytes)}`)
      )
      results.appendChild(
        button(
          "Delete previewed files",
          async () => {
            const outcome = await invoke("everyday_apply_cleanup", {
              request: { confirm: true },
            })
            status.textContent =
              outcome.status === "applied"
                ? `Removed ${formatBytes(outcome.bytes_removed)}.`
                : outcome.message || outcome.detail
            await refresh()
          },
          { ariaLabel: "Confirm disk cleanup" }
        )
      )

      results.appendChild(el("h2", null, "Termiche"))
      results.appendChild(
        el(
          "p",
          null,
          `Livello ${overview.thermal.level}. CPU ${overview.thermal.cpu_temp_celsius ?? "n/a"} / GPU ${overview.thermal.gpu_temp_celsius ?? "n/a"} C`
        )
      )
      status.textContent = "Panoramica quotidiana pronta."
    } catch (error) {
      status.textContent = `Error: ${error}`
    }
  }

  root.appendChild(
    button("Refresh", refresh, { ariaLabel: "Refresh everyday", className: "btn btn-large" })
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
  const status = el("p", "status", "Loading...")
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
      ? `Overlay enabled (${overview.method_id})`
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
        `Pannello: ${panel.status}; FPS ${panel.avg_fps ?? "n/a"}; frame ${panel.avg_frame_time_ms ?? "n/a"} ms.`
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
      status.textContent = `Error: ${error}`
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
          status.textContent = `Salvataggio failed: ${error}`
        }
      },
      { ariaLabel: "Save overlay" }
    )
  )
  row.appendChild(
    button(
      "Disattiva globalmente",
      async () => {
        try {
          applyOverview(await invoke("overlay_disable_globally"))
        } catch (error) {
          status.textContent = `Error: ${error}`
        }
      },
      { ariaLabel: "Disattiva overlay globalmente", className: "btn btn-secondary" }
    )
  )
  row.appendChild(button("Refresh", refresh, { ariaLabel: "Refresh overlay" }))
  root.appendChild(row)
  await refresh()
}

const setHashQuiet = (hashPath) => {
  if (window.location.hash === hashPath) {
    return
  }
  ignoreHashChange = true
  window.location.hash = hashPath
  queueMicrotask(() => {
    ignoreHashChange = false
  })
}

const renderRoute = async (routes, info) => {
  const token = ++renderToken
  const id = parseRouteId()
  const route = routes.find((item) => item.id === id) || routes[0]
  const activeId = route.id
  setHashQuiet(route.hash_path)
  renderNav(routes, activeId)
  updateStepStrip(activeId)
  document.querySelector("#title").textContent = route.title
  const body = document.querySelector("#body")
  body.replaceChildren()
  try {
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
  } catch (error) {
    if (token !== renderToken) {
      return
    }
    body.replaceChildren()
    body.appendChild(el("p", "status", `Page error: ${error}`))
    body.appendChild(
      button("Retry", () => handleNavigate(activeId), {
        ariaLabel: "Retry page",
        className: "btn",
      })
    )
  }
  if (token !== renderToken) {
    return
  }
  document.querySelector("#meta").textContent =
    `Unprivileged shell v${info.app_version} (core ${info.core_version}). No elevation at startup.`
  document.querySelector("#main").focus({ preventScroll: true })
}

const handleNavigate = async (id) => {
  try {
    const hashPath = await invoke("open_route", { id })
    setHashQuiet(hashPath)
  } catch (error) {
    setHashQuiet(`#/${id}`)
    console.warn("open_route failed", error)
  }
  await renderRoute(shellRoutes, shellInfo)
}

window.addEventListener("hashchange", () => {
  if (!shellInfo || ignoreHashChange) {
    return
  }
  renderRoute(shellRoutes, shellInfo)
})

window.addEventListener("DOMContentLoaded", async () => {
  const body = document.querySelector("#body")
  try {
    shellInfo = await invoke("get_shell_info")
    shellRoutes = shellInfo.routes
    if (!window.location.hash) {
      setHashQuiet(shellRoutes[0].hash_path)
    }
    await renderRoute(shellRoutes, shellInfo)
  } catch (error) {
    if (body) {
      body.replaceChildren()
      body.appendChild(el("p", "status", `Startup failed: ${error}`))
    }
  }
})
