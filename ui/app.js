const { invoke } = window.__TAURI__.core

const PLACEHOLDER = {
  actions:
    "Browse optimization actions with apply, restore and verify. Explanations and elevation prompts land in the actions UI task.",
  profiles:
    "Detect games from documented paths and apply profiles on start. Profile editing UI lands in the profiles task.",
  changelog:
    "Review time, action, before/after and restore status from the local change log.",
  "restore-all":
    "Revert every outstanding Hertzline change in one step. Uses the restore-all path from the rollback framework.",
}

let shellRoutes = []
let shellInfo = null
let lastMeasure = null
let beforeBench = null
let afterBench = null
let lastVerdict = null

const parseRouteId = () => {
  const hash = window.location.hash.replace(/^#\/?/, "")
  return hash || "measure"
}

const formatMetric = (metric) => {
  if (!metric || !metric.status) {
    return "unavailable"
  }
  if (metric.status === "available") {
    const unit = metric.unit ? ` ${metric.unit}` : ""
    return `${metric.value}${unit}`
  }
  return `unavailable (${metric.reason || "no reason"})`
}

const formatTextMetric = (metric) => {
  if (!metric || !metric.status) {
    return "unavailable"
  }
  if (metric.status === "available") {
    return metric.value
  }
  return `unavailable (${metric.reason || "no reason"})`
}

const formatSpread = (spread) => {
  if (!spread) {
    return "n/a"
  }
  return `median ${spread.median.toFixed(3)} (IQR ${spread.iqr.toFixed(3)}; ${spread.min.toFixed(3)}--${spread.max.toFixed(3)})`
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

const renderMetricTable = (session) => {
  const table = el("table", "metrics")
  table.setAttribute("aria-label", "Measurement results")
  const body = document.createElement("tbody")
  const rows = [
    ["avg FPS", formatMetric(session.frame.avg_fps)],
    ["1% low FPS", formatMetric(session.frame.low_1pct_fps)],
    ["0.1% low FPS", formatMetric(session.frame.low_0_1pct_fps)],
    ["frame-time variance", formatMetric(session.frame.frame_time_variance_ms2)],
    ["CPU usage", formatMetric(session.hardware.cpu_usage_percent)],
    ["GPU usage", formatMetric(session.hardware.gpu_usage_percent)],
    ["RAM used", formatMetric(session.hardware.ram_used_bytes)],
    ["VRAM used", formatMetric(session.hardware.vram_used_bytes)],
    ["CPU temp", formatMetric(session.hardware.cpu_temp_celsius)],
    ["GPU temp", formatMetric(session.hardware.gpu_temp_celsius)],
    ["throttling", formatTextMetric(session.hardware.throttling)],
    ["disk read", formatMetric(session.disk.disk_read_bytes_per_sec)],
    ["disk write", formatMetric(session.disk.disk_write_bytes_per_sec)],
    ["disk queue", formatMetric(session.disk.disk_queue_depth)],
    ["network host", formatTextMetric(session.network.network_host)],
    ["latency", formatMetric(session.network.network_latency_ms)],
    ["jitter", formatMetric(session.network.network_jitter_ms)],
    ["packet loss", formatMetric(session.network.network_packet_loss_percent)],
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

const renderMeasure = (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Capture one local session. Missing sensors stay unavailable. Export uses identical CSV and JSON fields."
    )
  )

  const processInput = el("input")
  processInput.type = "text"
  processInput.id = "measure-process"
  processInput.setAttribute("aria-label", "Target process name")
  processInput.placeholder = "optional process label"

  const hostInput = el("input")
  hostInput.type = "text"
  hostInput.id = "measure-host"
  hostInput.setAttribute("aria-label", "Network host for latency")
  hostInput.placeholder = "optional ping host"

  const csvInput = el("textarea")
  csvInput.id = "measure-presentmon"
  csvInput.rows = 4
  csvInput.setAttribute("aria-label", "PresentMon CSV")
  csvInput.placeholder = "Optional PresentMon CSV (MsBetweenPresents) when probe is available"

  const mangoInput = el("textarea")
  mangoInput.id = "measure-mangohud"
  mangoInput.rows = 4
  mangoInput.setAttribute("aria-label", "MangoHud log")
  mangoInput.placeholder = "Optional MangoHud frametime log when probe is available"

  const form = el("div", "panel")
  form.appendChild(field("Target process", processInput))
  form.appendChild(field("Network host", hostInput))
  form.appendChild(field("PresentMon CSV", csvInput))
  form.appendChild(field("MangoHud log", mangoInput))

  const status = el("p", "status", "")
  status.id = "measure-status"
  status.setAttribute("role", "status")

  const results = el("div", "results")
  results.id = "measure-results"

  const handleRun = async () => {
    status.textContent = "Running measurement..."
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
      status.textContent = `Session ${lastMeasure.session.meta.session_id}. Probe: ${lastMeasure.frame_probe.status} (${lastMeasure.frame_probe.detail})`
      results.appendChild(renderMetricTable(lastMeasure.session))
      results.appendChild(el("p", "note", lastMeasure.local_measurement_note))
      const exports = el("div", "actions-row")
      exports.appendChild(
        button("Export CSV", () => {
          downloadText(
            `${lastMeasure.session.meta.session_id}.csv`,
            lastMeasure.csv,
            "text/csv"
          )
        }, { ariaLabel: "Export measurement as CSV" })
      )
      exports.appendChild(
        button("Export JSON", () => {
          downloadText(
            `${lastMeasure.session.meta.session_id}.json`,
            lastMeasure.json,
            "application/json"
          )
        }, { ariaLabel: "Export measurement as JSON" })
      )
      results.appendChild(exports)
    } catch (error) {
      status.textContent = `Measurement failed: ${error}`
    }
  }

  form.appendChild(button("Run measurement", handleRun, { ariaLabel: "Run measurement session" }))
  root.appendChild(form)
  root.appendChild(status)
  root.appendChild(results)

  if (lastMeasure) {
    status.textContent = `Last session ${lastMeasure.session.meta.session_id}`
    results.appendChild(renderMetricTable(lastMeasure.session))
    results.appendChild(el("p", "note", lastMeasure.local_measurement_note))
  }
}

const renderBenchCard = (title, result) => {
  const card = el("div", "panel")
  card.appendChild(el("h2", null, title))
  if (!result) {
    card.appendChild(el("p", "muted", "Not run yet."))
    return card
  }
  card.appendChild(el("p", null, `Source: ${result.source}`))
  card.appendChild(el("p", null, `avg FPS ${formatSpread(result.avg_fps)}`))
  card.appendChild(el("p", null, `1% low ${formatSpread(result.low_1pct_fps)}`))
  card.appendChild(el("p", null, `0.1% low ${formatSpread(result.low_0_1pct_fps)}`))
  card.appendChild(el("p", null, `frame time ${formatSpread(result.avg_frame_time_ms)}`))
  return card
}

const parseWindowsJson = (text) => {
  const parsed = JSON.parse(text)
  if (!Array.isArray(parsed) || parsed.length === 0) {
    throw new Error("windows must be a non-empty JSON array of frame-time arrays")
  }
  return parsed
}

const renderBenchmark = async (root) => {
  const defaults = await invoke("benchmark_default_config")
  const noLocal = await invoke("benchmark_no_local_note")

  root.appendChild(
    el(
      "p",
      "lede",
      "Before/after benchmark with warm-up, fixed duration, and at least 3 passes. A change is helpful only if the measured gain exceeds the noise band."
    )
  )
  root.appendChild(el("p", "note", noLocal))

  const warmup = el("input")
  warmup.type = "number"
  warmup.id = "bench-warmup"
  warmup.min = "1"
  warmup.value = String(defaults.warmup_ms)
  warmup.setAttribute("aria-label", "Warm-up milliseconds")

  const measureMs = el("input")
  measureMs.type = "number"
  measureMs.id = "bench-measure"
  measureMs.min = "1"
  measureMs.value = String(defaults.measure_ms)
  measureMs.setAttribute("aria-label", "Measure milliseconds per pass")

  const passes = el("input")
  passes.type = "number"
  passes.id = "bench-passes"
  passes.min = "3"
  passes.value = String(defaults.pass_count)
  passes.setAttribute("aria-label", "Pass count")

  const windowsInput = el("textarea")
  windowsInput.id = "bench-windows"
  windowsInput.rows = 5
  windowsInput.setAttribute("aria-label", "Frame time windows JSON")
  windowsInput.placeholder =
    'JSON: [[warmup ms...],[pass1...],[pass2...],[pass3...]] — local frame times only'

  const form = el("div", "panel")
  form.appendChild(field("Warm-up (ms)", warmup))
  form.appendChild(field("Measure per pass (ms)", measureMs))
  form.appendChild(field("Passes (>=3)", passes))
  form.appendChild(field("Frame windows (JSON)", windowsInput))

  const status = el("p", "status", "")
  status.id = "bench-status"
  status.setAttribute("role", "status")

  const results = el("div", "results")
  results.id = "bench-results"

  const readConfig = () => ({
    warmup_ms: Number(warmup.value),
    measure_ms: Number(measureMs.value),
    pass_count: Number(passes.value),
  })

  const refreshResults = () => {
    results.replaceChildren()
    results.appendChild(renderBenchCard("Before", beforeBench))
    results.appendChild(renderBenchCard("After", afterBench))
    if (lastVerdict) {
      const v = el("div", "panel verdict")
      v.appendChild(el("h2", null, "Verdict"))
      v.appendChild(el("p", null, lastVerdict.message))
      v.appendChild(
        el(
          "p",
          null,
          `Measured median gain ${lastVerdict.gain.toFixed(3)} fps (noise band ${lastVerdict.noise_band.toFixed(3)}). Before ${lastVerdict.before_median.toFixed(3)} -> after ${lastVerdict.after_median.toFixed(3)}.`
        )
      )
      v.appendChild(el("p", "note", lastVerdict.explanation))
      if (lastVerdict.offer_revert) {
        v.appendChild(
          el(
            "p",
            "revert",
            "Offer revert: yes. Restore the change if you applied one (Restore all route)."
          )
        )
      }
      results.appendChild(v)
    }
  }

  const handlePhase = async (phase) => {
    status.textContent = `Running ${phase}...`
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
      status.textContent = `${phase} complete (source ${result.source}).`
      refreshResults()
    } catch (error) {
      status.textContent = `${phase} failed: ${error}`
    }
  }

  const handleCompare = async () => {
    if (!beforeBench || !afterBench) {
      status.textContent = "Run before and after first."
      return
    }
    try {
      lastVerdict = await invoke("benchmark_compare", {
        before: beforeBench,
        after: afterBench,
      })
      status.textContent = `Verdict: ${lastVerdict.message}`
      refreshResults()
    } catch (error) {
      status.textContent = `Compare failed: ${error}`
    }
  }

  const handleMock = async () => {
    status.textContent = "Running labeled mock pair (not a live capture)..."
    try {
      const pair = await invoke("benchmark_run_labeled_mock", {
        beforeFrameMs: 1000 / 60,
        afterFrameMs: 1000 / 60,
      })
      beforeBench = pair[0]
      afterBench = pair[1]
      lastVerdict = pair[2]
      status.textContent =
        "Labeled mock complete. Source is labeled_mock -- not a live capture. No marketing +X FPS claim."
      refreshResults()
    } catch (error) {
      status.textContent = `Mock failed: ${error}`
    }
  }

  const actions = el("div", "actions-row")
  actions.appendChild(
    button("Run before", () => handlePhase("before"), { ariaLabel: "Run before benchmark" })
  )
  actions.appendChild(
    button("Run after", () => handlePhase("after"), { ariaLabel: "Run after benchmark" })
  )
  actions.appendChild(
    button("Compare", handleCompare, { ariaLabel: "Compare before and after" })
  )
  actions.appendChild(
    button("Labeled mock (no effect)", handleMock, {
      ariaLabel: "Run labeled mock benchmark pair",
      className: "btn btn-secondary",
    })
  )

  form.appendChild(actions)
  root.appendChild(form)
  root.appendChild(status)
  root.appendChild(results)
  refreshResults()
}

const renderPlaceholder = (root, id) => {
  root.appendChild(el("p", "lede", PLACEHOLDER[id] || "This shell route is registered."))
}

const renderRoute = async (routes, info) => {
  const id = parseRouteId()
  const route = routes.find((item) => item.id === id) || routes[0]
  const activeId = route.id
  if (window.location.hash !== route.hash_path) {
    window.location.hash = route.hash_path
  }
  renderNav(routes, activeId)
  document.querySelector("#title").textContent = route.title
  const body = document.querySelector("#body")
  body.replaceChildren()
  if (activeId === "measure") {
    renderMeasure(body)
  } else if (activeId === "benchmark") {
    await renderBenchmark(body)
  } else {
    renderPlaceholder(body, activeId)
  }
  document.querySelector("#meta").textContent =
    `Unprivileged shell v${info.app_version} (core ${info.core_version}). No elevation at startup.`
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
