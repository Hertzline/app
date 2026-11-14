const { invoke } = window.__TAURI__.core

const COPY = {
  measure:
    "Start a measurement session. Frame times, hardware, disk and network collectors wire up in the measure UI task.",
  benchmark:
    "Run a fixed-duration before/after benchmark. Pass counts, noise band and verdicts land in the benchmark UI task.",
  actions:
    "Browse optimization actions with apply, restore and verify. Explanations and elevation prompts land in the actions UI task.",
  profiles:
    "Detect games from documented paths and apply profiles on start. Profile editing UI lands in the profiles task.",
  changelog:
    "Review time, action, before/after and restore status from the local change log.",
  "restore-all":
    "Revert every outstanding Hertzline change in one step. Uses the restore-all path from the rollback framework.",
}

const parseRouteId = () => {
  const hash = window.location.hash.replace(/^#\/?/, "")
  return hash || "measure"
}

const renderNav = (routes, activeId) => {
  const nav = document.querySelector("#nav")
  nav.replaceChildren()
  for (const route of routes) {
    const link = document.createElement("a")
    link.href = route.hash_path
    link.textContent = route.title
    link.dataset.route = route.id
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

const renderRoute = (routes, info) => {
  const id = parseRouteId()
  const route = routes.find((item) => item.id === id) || routes[0]
  const activeId = route.id
  if (window.location.hash !== route.hash_path) {
    window.location.hash = route.hash_path
  }
  renderNav(routes, activeId)
  document.querySelector("#title").textContent = route.title
  document.querySelector("#body").textContent =
    COPY[activeId] || "This shell route is registered."
  document.querySelector("#meta").textContent =
    `Unprivileged shell v${info.app_version} (core ${info.core_version}). No elevation at startup.`
  document.querySelector("#main").focus({ preventScroll: true })
}

let shellRoutes = []
let shellInfo = null

const handleNavigate = async (id) => {
  const hashPath = await invoke("open_route", { id })
  if (window.location.hash !== hashPath) {
    window.location.hash = hashPath
  }
  renderRoute(shellRoutes, shellInfo)
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
  renderRoute(shellRoutes, shellInfo)
})
