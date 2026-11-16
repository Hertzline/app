<div align="center">

<a href="https://github.com/Hertzline/app"><img src="https://capsule-render.vercel.app/api?type=waving&height=180&color=0:0f1412,55:12352c,100:0f6b4c&text=Hertzline&fontColor=e8eeeb&fontSize=64&fontAlignY=45&animation=twinkling" alt="Hertzline" width="100%" /></a>

<img src="https://readme-typing-svg.demolab.com?font=IBM+Plex+Mono&weight=500&size=16&color=2A9B7A&center=true&vCenter=true&width=520&height=34&duration=2800&pause=1200&lines=Desktop+shell+(Tauri+2);Unprivileged+by+default;Measure.+Explain.+Restore.;No+unverified+FPS+claims" alt="Desktop shell" />

<p>
  <a href="https://github.com/Hertzline"><img src="https://skillicons.dev/icons?i=github&theme=dark" width="44" height="44" alt="GitHub" /></a>&nbsp;
  <a href="https://www.rust-lang.org"><img src="https://skillicons.dev/icons?i=rust&theme=dark" width="44" height="44" alt="Rust" /></a>&nbsp;
  <a href="https://tauri.app"><img src="https://skillicons.dev/icons?i=tauri&theme=dark" width="44" height="44" alt="Tauri" /></a>
</p>

</div>

<br/>

<img src="https://capsule-render.vercel.app/api?type=rect&height=3&color=0:0f1412,35:2a9b7a,70:c9a227,100:0f1412" width="100%" alt="" />

<br/>

<p align="center"><img src="https://readme-typing-svg.demolab.com?font=IBM+Plex+Mono&weight=700&size=18&color=2A9B7A&center=true&vCenter=true&width=420&height=40&duration=1&pause=1000000&letterSpacing=6px&lines=WHAT+THIS+REPO+IS" alt="What this repo is" /></p>

Desktop shell for [Hertzline](https://github.com/Hertzline). Unprivileged by default. Elevation is requested per action only, never for the whole process.

Requires sibling checkout of `Hertzline/core` at `../core`.

<img src="https://capsule-render.vercel.app/api?type=rect&height=3&color=0:0f1412,35:2a9b7a,70:c9a227,100:0f1412" width="100%" alt="" />

<br/>

<p align="center"><img src="https://readme-typing-svg.demolab.com?font=IBM+Plex+Mono&weight=700&size=18&color=2A9B7A&center=true&vCenter=true&width=420&height=40&duration=1&pause=1000000&letterSpacing=6px&lines=LAYOUT" alt="Layout" /></p>

| Path | Role |
|---|---|
| `crates/hertzline-app` | Profile lifecycle, shell, measure, benchmark, actions hosts |
| `src-tauri` | Tauri 2 binary (`hertzline`) |
| `ui` | Static shell UI (hash routes) |

Routes: `#/home` · `#/measure` · `#/benchmark` · `#/actions` · `#/profiles` · `#/everyday` · `#/overlay` · `#/changelog` · `#/restore-all`

<img src="https://capsule-render.vercel.app/api?type=rect&height=3&color=0:0f1412,35:2a9b7a,70:c9a227,100:0f1412" width="100%" alt="" />

<br/>

<p align="center"><img src="https://readme-typing-svg.demolab.com?font=IBM+Plex+Mono&weight=700&size=18&color=2A9B7A&center=true&vCenter=true&width=420&height=40&duration=1&pause=1000000&letterSpacing=6px&lines=VERSIONS" alt="Versions" /></p>

| Package | Version | License |
|---|---|---|
| tauri | 2.12.5 | Apache-2.0 OR MIT |
| tauri-build | 2.7.4 | Apache-2.0 OR MIT |
| @tauri-apps/cli | 2.12.4 | Apache-2.0 OR MIT |

<img src="https://capsule-render.vercel.app/api?type=rect&height=3&color=0:0f1412,35:2a9b7a,70:c9a227,100:0f1412" width="100%" alt="" />

<br/>

<p align="center"><img src="https://readme-typing-svg.demolab.com?font=IBM+Plex+Mono&weight=700&size=18&color=2A9B7A&center=true&vCenter=true&width=420&height=40&duration=1&pause=1000000&letterSpacing=6px&lines=BUILD" alt="Build" /></p>

```bash
npm ci
cargo test --locked
npm run build
```

Headless smoke:

```bash
set HERTZLINE_APP_SMOKE=1
target\release\hertzline.exe
```

<p align="center"><sub><i>Clean measurements are like a well-refereed match: when everything works, nobody invents the score.</i></sub></p>

## License

[MIT](LICENSE) · [NOTICE](NOTICE)

<img src="https://capsule-render.vercel.app/api?type=waving&section=footer&height=110&color=0:0f1412,55:12352c,100:0f6b4c" alt="" width="100%" />
