const { invoke } = window.__TAURI__.core

let shellRoutes = []
let shellInfo = null
let lastMeasure = null
let beforeBench = null
let afterBench = null
let lastVerdict = null
let profilesState = null

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

const renderExplanation = (parent, explanation) => {
  const dl = el("dl", "explanation")
  const rows = [
    ["What", explanation.what],
    ["Expected effect", explanation.expected_effect],
    ["Risk", explanation.risk],
    ["How to undo", explanation.how_to_undo],
    ["Local measurement", explanation.local_measurement],
  ]
  for (const [label, value] of rows) {
    dl.appendChild(el("dt", null, label))
    dl.appendChild(el("dd", null, value))
  }
  parent.appendChild(dl)
}

const renderActions = async (root) => {
  const catalog = await invoke("actions_catalog")
  root.appendChild(
    el(
      "p",
      "lede",
      `Optimization actions for this host (${catalog.host_os}). Each action shows what it does, risk, and how to undo. Elevation is requested per action only.`
    )
  )
  root.appendChild(el("p", "note", catalog.local_measurement_note))

  if (catalog.macos_honesty) {
    const honesty = el("div", "panel honesty")
    honesty.appendChild(el("h2", null, "macOS honesty"))
    honesty.appendChild(el("p", null, catalog.macos_honesty.banner))
    if (catalog.host_os === "macos") {
      const list = el("ul", "cap-list")
      for (const cap of catalog.macos_honesty.capabilities.filter((c) => c.kind === "action")) {
        const item = el(
          "li",
          null,
          `${cap.id} [${cap.level}] toggle=${cap.exposes_toggle ? "yes" : "no"} — ${cap.summary}`
        )
        list.appendChild(item)
      }
      honesty.appendChild(list)
    }
    root.appendChild(honesty)
  }

  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const hostActions = catalog.actions.filter((a) => a.host_status === "available")
  const otherActions = catalog.actions.filter((a) => a.host_status !== "available")

  const renderCard = (action) => {
    const panel = el("div", "panel action-card")
    panel.appendChild(el("h2", null, action.title))
    panel.appendChild(
      el(
        "p",
        "muted",
        `${action.id} · ${action.os} · ${action.ac_refs.join(", ")} · ${action.kind}${
          action.requires_elevation ? " · elevation required" : ""
        }`
      )
    )
    if (action.current_state) {
      panel.appendChild(el("p", null, `Current: ${action.current_state}`))
    }
    renderExplanation(panel, action.explanation)

    if (action.host_status !== "available") {
      panel.appendChild(el("p", "note", `Not available on this host (${action.host_status}).`))
      return panel
    }

    const input = el("input")
    input.type = "text"
    input.setAttribute("aria-label", action.input_hint || `Input for ${action.title}`)
    input.placeholder = action.input_hint || "optional input"
    input.autocomplete = "off"
    if (action.kind === "needs_input" || action.kind === "toggle" || action.kind === "destructive") {
      if (action.input_hint) {
        panel.appendChild(field(action.input_hint, input))
      }
    }

    const elevCheck = el("input")
    elevCheck.type = "checkbox"
    elevCheck.id = `elev-${action.id}`
    elevCheck.setAttribute("aria-label", `Confirm elevation for ${action.title}`)
    const elevLabel = el("label", "check")
    elevLabel.appendChild(elevCheck)
    elevLabel.appendChild(
      document.createTextNode(
        action.elevation_reason ||
          "I confirm a per-action elevation prompt (not whole-app elevation)."
      )
    )
    if (action.requires_elevation) {
      panel.appendChild(elevLabel)
    }

    const destCheck = el("input")
    destCheck.type = "checkbox"
    destCheck.setAttribute("aria-label", `Confirm destructive cleanup for ${action.title}`)
    const destLabel = el("label", "check")
    destLabel.appendChild(destCheck)
    destLabel.appendChild(
      document.createTextNode("I confirm cleanup after reviewing the size preview.")
    )
    if (action.kind === "destructive") {
      panel.appendChild(destLabel)
    }

    const row = el("div", "actions-row")
    if (action.exposes_switch || action.kind === "report" || action.kind === "destructive") {
      row.appendChild(
        button(
          action.kind === "report" ? "Refresh report" : "Apply",
          async () => {
            status.textContent = `Applying ${action.id}…`
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
                status.textContent = `Applied ${outcome.action}: ${outcome.before} → ${outcome.after} (entry ${outcome.entry_id})`
              } else {
                status.textContent = `Failed ${outcome.action}: ${outcome.detail}`
              }
            } catch (error) {
              status.textContent = `Apply failed: ${error}`
            }
          },
          { ariaLabel: `Apply ${action.title}` }
        )
      )
    }
    panel.appendChild(row)
    return panel
  }

  root.appendChild(el("h2", null, "Available on this host"))
  if (hostActions.length === 0) {
    root.appendChild(el("p", "muted", "No host actions registered for this OS."))
  }
  for (const action of hostActions) {
    root.appendChild(renderCard(action))
  }

  const other = el("details", "other-os")
  other.appendChild(el("summary", null, `Other OS actions (${otherActions.length})`))
  for (const action of otherActions) {
    other.appendChild(renderCard(action))
  }
  root.appendChild(other)
  root.appendChild(status)
}

const renderProfiles = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Per-game profiles apply a saved action set when the game starts and restore every change on exit. Detection uses documented launcher paths only."
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
      activePanel.appendChild(el("h2", null, "Active session"))
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
              status.textContent = "Restoring active profile…"
              try {
                const outcome = await invoke("profiles_restore", { labeledMock: false })
                if (outcome.status === "restored") {
                  status.textContent = `Restored ${outcome.name} (${outcome.game_key}).`
                } else {
                  status.textContent = `Restore failed: ${outcome.detail}`
                }
                await refresh()
              } catch (error) {
                status.textContent = `Restore failed: ${error}`
              }
            },
            { ariaLabel: "Restore active profile on game exit" }
          )
        )
      } else {
        activePanel.appendChild(el("p", "muted", "No profile session is active."))
      }
      results.appendChild(activePanel)

      const manualPanel = el("div", "panel")
      manualPanel.appendChild(el("h2", null, "Add manual path"))
      const nameInput = el("input")
      nameInput.type = "text"
      nameInput.id = "manual-name"
      nameInput.setAttribute("aria-label", "Manual game name")
      nameInput.autocomplete = "off"
      const pathInput = el("input")
      pathInput.type = "text"
      pathInput.id = "manual-path"
      pathInput.setAttribute("aria-label", "Manual install path")
      pathInput.autocomplete = "off"
      manualPanel.appendChild(field("Game name", nameInput))
      manualPanel.appendChild(field("Install path", pathInput))
      manualPanel.appendChild(
        button(
          "Add manual path",
          async () => {
            status.textContent = "Adding manual path…"
            try {
              profilesState = await invoke("profiles_add_manual", {
                request: {
                  name: nameInput.value,
                  install_path: pathInput.value,
                },
              })
              status.textContent = "Manual path added."
              nameInput.value = ""
              pathInput.value = ""
              await refresh()
            } catch (error) {
              status.textContent = `Manual path failed: ${error}`
            }
          },
          { ariaLabel: "Add manual game install path" }
        )
      )
      results.appendChild(manualPanel)

      results.appendChild(el("h2", null, `Detected games (${profilesState.games.length})`))
      if (!profilesState.games.length) {
        results.appendChild(
          el("p", "muted", "No games found yet. Add a manual path or install a supported launcher.")
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
        actionsInput.setAttribute(
          "aria-label",
          `Action ids for ${game.name} (comma-separated)`
        )
        actionsInput.placeholder = "action ids, comma-separated (e.g. windows.game_mode)"
        panel.appendChild(field("Profile actions", actionsInput))
        const row = el("div", "actions-row")
        row.appendChild(
          button(
            "Save profile",
            async () => {
              status.textContent = `Saving profile for ${game.game_key}…`
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
            { ariaLabel: `Save profile for ${game.name}`, className: "btn btn-secondary" }
          )
        )
        row.appendChild(
          button(
            "Apply on start",
            async () => {
              status.textContent = `Applying profile for ${game.game_key}…`
              try {
                const outcome = await invoke("profiles_apply", {
                  request: {
                    game_key: game.game_key,
                    labeled_mock: false,
                  },
                })
                if (outcome.status === "applied") {
                  status.textContent = `Applied ${outcome.name}: ${outcome.action_ids.join(", ")}`
                } else {
                  status.textContent = `Apply failed: ${outcome.detail}`
                }
                await refresh()
              } catch (error) {
                status.textContent = `Apply failed: ${error}`
              }
            },
            { ariaLabel: `Apply profile for ${game.name} on game start` }
          )
        )
        panel.appendChild(row)
        results.appendChild(panel)
      }
      status.textContent = `${profilesState.games.length} game(s), ${profilesState.profiles.length} profile(s).`
    } catch (error) {
      status.textContent = `Profiles failed: ${error}`
    }
  }

  root.appendChild(
    button("Refresh games and profiles", refresh, {
      ariaLabel: "Refresh detected games and profiles",
    })
  )
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderChangelog = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "Local change log: time, action, before/after, and restore status. Nothing leaves this machine."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const refresh = async () => {
    results.replaceChildren()
    try {
      const rows = await invoke("actions_change_log")
      if (!rows.length) {
        results.appendChild(el("p", "muted", "No changes recorded yet."))
        status.textContent = "Change log empty."
        return
      }
      const table = el("table", "metrics")
      table.setAttribute("aria-label", "Change log")
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
                status.textContent = `Restoring entry ${row.id}…`
                try {
                  const updated = await invoke("actions_restore_entry", { entryId: row.id })
                  status.textContent = `Restored entry ${updated.id} (${updated.restore_status}).`
                  await refresh()
                } catch (error) {
                  status.textContent = `Restore failed: ${error}`
                }
              },
              { ariaLabel: `Restore change ${row.id}`, className: "btn btn-secondary" }
            )
          )
        }
        tr.appendChild(cell)
        body.appendChild(tr)
      }
      table.appendChild(body)
      results.appendChild(table)
      status.textContent = `${rows.length} change log row(s).`
    } catch (error) {
      status.textContent = `Change log failed: ${error}`
    }
  }

  root.appendChild(
    button("Refresh change log", refresh, { ariaLabel: "Refresh change log" })
  )
  root.appendChild(status)
  root.appendChild(results)
  await refresh()
}

const renderRestoreAll = async (root) => {
  root.appendChild(
    el(
      "p",
      "lede",
      "One-click restore all reverts every outstanding Hertzline change. This path also runs on crash recovery and uninstall without the UI."
    )
  )
  const status = el("p", "status", "")
  status.setAttribute("role", "status")
  const results = el("div", "results")

  const handleRestore = async () => {
    status.textContent = "Running restore all…"
    results.replaceChildren()
    try {
      const report = await invoke("actions_restore_all")
      status.textContent = `Restore all ${report.status} (trigger ${report.trigger}; ok=${report.ok}; requires_ui=${report.requires_ui}).`
      const list = el("ul", "cap-list")
      for (const item of report.items) {
        list.appendChild(el("li", null, item))
      }
      results.appendChild(list)
      results.appendChild(el("p", "note", `Store: ${report.store_path}`))
    } catch (error) {
      status.textContent = `Restore all failed: ${error}`
    }
  }

  root.appendChild(
    button("Restore all outstanding changes", handleRestore, {
      ariaLabel: "Restore all outstanding Hertzline changes",
    })
  )
  root.appendChild(status)
  root.appendChild(results)
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
  } else if (activeId === "actions") {
    await renderActions(body)
  } else if (activeId === "profiles") {
    await renderProfiles(body)
  } else if (activeId === "changelog") {
    await renderChangelog(body)
  } else if (activeId === "restore-all") {
    await renderRestoreAll(body)
  } else {
    body.appendChild(el("p", "lede", "This shell route is registered."))
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
