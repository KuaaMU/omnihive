<p align="center">
  <img src="app/src-tauri/icons/icon.png" width="120" alt="Omnihive Logo" />
</p>

<h1 align="center">Omnihive</h1>

<p align="center">
  <strong>One prompt, infinite agents.</strong><br/>
  Seed Prompt &rarr; Agent Swarm &rarr; Autonomous Loop &rarr; Self-Evolving Company
</p>

<p align="center">
  <a href="https://github.com/KuaaMU/omnihive/releases"><img src="https://img.shields.io/github/v/release/KuaaMU/omnihive?style=flat-square&color=blue" alt="Release" /></a>
  <a href="https://github.com/KuaaMU/omnihive/blob/main/LICENSE"><img src="https://img.shields.io/github/license/KuaaMU/omnihive?style=flat-square" alt="License" /></a>
  <a href="https://github.com/KuaaMU/omnihive/stargazers"><img src="https://img.shields.io/github/stars/KuaaMU/omnihive?style=flat-square" alt="Stars" /></a>
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-brightgreen?style=flat-square" alt="Platform" />
  <img src="https://img.shields.io/badge/built%20with-Tauri%202%20%2B%20React-orange?style=flat-square" alt="Tech" />
</p>

---

## What is Omnihive?

Omnihive is a **desktop application** that bootstraps fully autonomous AI companies from a single seed prompt. It orchestrates multi-agent swarms powered by Claude Code, Codex CLI, or OpenCode, running 24/7 autonomous loops with shared consensus memory.

**Think of it as:** a visual control tower for spawning and managing AI agent hives that build real software products.

```
"Build a time-tracking SaaS for freelancers"
                    |
          Omnihive analyzes domain
                    |
   Generates 12-agent swarm (CEO, CTO, Fullstack, DevOps, QA...)
                    |
     Starts autonomous build loop with consensus
                    |
   Agents collaborate, review, deploy, self-evolve
```

## Key Features

### Agent Orchestration
- **One-click bootstrap** - describe your idea, get a complete AI company with agents, skills, and workflows
- **12+ specialized roles** - CEO, CTO, Fullstack, DevOps, QA, Marketing, Design, and more
- **Multi-engine support** - Claude Code, Codex CLI, OpenCode with automatic failover
- **Consensus-driven** - agents share state through a consensus memory document
- **Cycle history & logs** - real-time monitoring of agent activity

### Interactive Swarm Visualization (v0.3.0)
- **Agent Topology Ring** - SVG ring visualization showing all agents, click any agent to inspect details
- **Agent Detail Panel** - view per-agent performance (cycles, errors, success rate), recent activity, and memory
- **Expandable Cycle Timeline** - click any cycle to see outcome, files changed, duration, and error details
- **Activity Feed** - color-coded real-time event stream with agent badges
- **Terminal Log Viewer** - searchable log output with auto-scroll and syntax highlighting (errors/warnings)

### Configurable Runtime (v0.3.0)
- **Per-project engine/model** - override global engine and model settings on each project independently
- **Flexible API format** - supports Anthropic Standard, Claude Code Compatible, and OpenAI Compatible formats
- **SSE streaming** - optional streaming mode for API responses with real-time token delivery
- **Custom headers** - configure `anthropic-version`, extra headers, and advanced provider options
- **API test command** - test provider connectivity directly from settings

### Resource Library
- **40+ built-in skills** - from coding standards to deployment patterns
- **Pre-built personas** - battle-tested agent configurations for every role
- **Workflow chains** - coordinated multi-agent execution sequences
- **MCP integration** - connect agents to external services (GitHub, Slack, databases, web search)
- **Remote repositories** - browse and install skills from any GitHub repo
- **Library management** - add custom skills, agents, and workflows directly from the UI

### Developer Experience
- **6 color themes** - 3 dark (Obsidian, Cyber, Ember) + 3 light (Daylight, Paper, Lavender)
- **Auto-detect providers** - scans your system for existing API keys (Anthropic, OpenAI, OpenRouter)
- **System diagnostics** - detects installed CLI tools, shells, and environments
- **Bilingual UI** - English & Chinese
- **Auto-update** - built-in update checker with one-click download

## Screenshots

| Dashboard | Project Detail | Library |
|:---------:|:--------------:|:-------:|
| Manage all projects with per-project config | Interactive agent topology & timeline | Browse skills & agents |

| Settings | Theme Selector | Agent Detail |
|:--------:|:--------------:|:------------:|
| Advanced provider management | 6 themes (dark + light) | Per-agent performance & memory |

## Quick Start

### Download

Grab the latest release from [GitHub Releases](https://github.com/KuaaMU/omnihive/releases):

- **Windows**: `Omnihive_x.x.x_x64-setup.exe` (NSIS) or `.msi`
- **macOS**: `.dmg` (Apple Silicon + Intel) - via CI/CD
- **Linux**: `.AppImage` / `.deb` - via CI/CD

### Prerequisites

You need at least one AI coding CLI installed:

| Engine | Install | Required? |
|--------|---------|-----------|
| [Claude Code](https://docs.anthropic.com/en/docs/claude-code) | `npm install -g @anthropic-ai/claude-code` | Recommended |
| [Codex CLI](https://github.com/openai/codex) | `npm install -g @openai/codex` | Optional |
| [OpenCode](https://github.com/opencode-ai/opencode) | `go install github.com/opencode-ai/opencode@latest` | Optional |

### First Run

1. Open Omnihive
2. Go to **Settings > AI Providers** > click "Detect Configurations" to auto-import your API keys
3. Go to **Settings > System** > click "Refresh" to verify your CLI tools are detected
4. Click **New Project**, enter your seed prompt, and follow the wizard
5. Hit **Start** on the project detail page to begin the autonomous loop

## Architecture

```
                    +-------------------+
                    |   Tauri Desktop   |
                    |   (Rust + React)  |
                    +--------+----------+
                             |
              +--------------+--------------+
              |              |              |
        +-----+----+  +-----+----+  +------+-----+
        | Bootstrap |  | Runtime  |  |  Library   |
        |  Engine   |  |  Loop    |  |  Manager   |
        +-----+----+  +-----+----+  +------+-----+
              |              |              |
         Seed Analysis   Agent Cycle    Skills/MCP/
         Config Gen      Consensus      Repo Browser
         File Gen        Log/Monitor    GitHub API
              |              |
        +-----+--------------+-----+
        |     Provider Router      |
        | Claude | Codex | OpenCode|
        |   (Standard / Stream)    |
        +--------------------------+
```

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop Framework | Tauri 2 |
| Backend | Rust (serde, ureq, chrono) |
| Frontend | React 18 + TypeScript |
| Styling | Tailwind CSS + CSS Variables |
| State Management | TanStack Query |
| Build | Vite + Cargo |
| Packaging | NSIS / MSI / DMG |

## Project Structure

> Phase 1 migration note: the new control-plane monorepo scaffolding now lives under `apps/`, `crates/`, `packages/`, `plugins/`, and `schemas/`.
> The current build path remains `app/` during incremental migration. See `docs/architecture/layers.md` for layer boundaries and dependency rules.

```
omnihive/
├── .github/                       # CI/CD & community
│   ├── workflows/build.yml        # Cross-platform build (Windows/macOS/Linux)
│   └── FUNDING.yml                # GitHub Sponsors
├── app/                           # Tauri desktop application
│   ├── src/                       # React frontend
│   │   ├── routes/                # Pages: Dashboard, NewProject, Library, Settings
│   │   ├── components/
│   │   │   ├── layout/            # Layout, Sidebar
│   │   │   ├── dashboard/         # ProjectCard, ConfigSelector
│   │   │   ├── project/           # AgentTopologyRing, CycleTimeline, ActivityFeed,
│   │   │   │                      # AgentDetailPanel, LogViewer, constants
│   │   │   └── library/           # PersonaDetail, SkillDetail, McpTab, RepoManager
│   │   ├── lib/                   # Types, Tauri bindings, i18n, utils
│   │   └── styles/                # Global CSS with 6 themes
│   ├── src-tauri/                 # Rust backend
│   │   ├── src/
│   │   │   ├── commands/          # Tauri commands
│   │   │   │   ├── bootstrap.rs   # Seed analysis & config generation
│   │   │   │   ├── runtime.rs     # Agent loop, per-project config, events
│   │   │   │   ├── library.rs     # Persona/skill/workflow listing
│   │   │   │   ├── settings.rs    # App settings CRUD
│   │   │   │   ├── mcp.rs         # MCP server management
│   │   │   │   ├── repo_manager.rs# GitHub repo browser & installer
│   │   │   │   ├── skill_manager.rs# Local skill scanner & custom add
│   │   │   │   ├── provider_detect.rs # Auto-detect API configs
│   │   │   │   └── system.rs      # System environment detection
│   │   │   ├── engine/
│   │   │   │   ├── api_client.rs  # Unified API: Anthropic/Stream/OpenAI
│   │   │   │   ├── bootstrap.rs   # Config bootstrap engine
│   │   │   │   └── generator.rs   # File generation engine
│   │   │   └── models/            # Shared data structures
│   │   └── Cargo.toml
│   └── package.json
├── library/                       # Built-in resource library
│   ├── personas/                  # 14 expert persona YAMLs
│   ├── skills/                    # Skill definitions (YAML)
│   ├── real-skills/               # Community skills (SKILL.md format)
│   ├── ecc-skills/                # ECC skills collection
│   ├── real-agents/               # Agent prompt templates (Markdown)
│   └── workflows/                 # Workflow chain YAMLs
├── CONTRIBUTING.md                # Contribution guide
├── LICENSE                        # MIT
└── README.md
```

## Changelog

### v0.3.0 - Runtime Overhaul + Interactive Visualization

**Runtime API Fix & Configurable Format**
- Rewrote `api_client.rs` with unified `ApiCallConfig` supporting Anthropic Standard, Claude Code Compatible, and OpenAI formats
- Added SSE streaming support (`force_stream`) for real-time API response parsing
- Configurable `anthropic-version` header and custom extra headers per provider
- Advanced provider settings UI (collapsible section in Settings)
- `test_api_call` command for provider connectivity testing
- Increased API error message truncation from 500 to 2000 chars for better debugging

**Per-Project Runtime Configuration**
- Projects can now override global engine and model settings independently
- `ConfigSelector` popover on Dashboard cards for quick per-project config
- Per-project overrides stored as `.runtime_override.json` in project directory
- Enhanced `ProjectCard` with inline start/stop, engine/model badges, status animation

**Interactive Project Detail Visualization**
- `AgentTopologyRing` - SVG ring with click-to-select agents, glow effects, center pulse hub
- `AgentDetailPanel` - per-agent performance stats, recent activity, memory viewer
- `CycleTimeline` - expandable cycle entries with outcome, files changed, duration, errors
- `ActivityFeed` - color-coded event stream with live polling
- `LogViewer` - terminal-style log with search filter, auto-scroll, error/warning highlighting
- New 2/3 + 1/3 responsive layout for Project Detail page
- In-memory event tracking system with `emit_project_event` and `get_project_events`

**Dashboard Enhancement**
- Rewritten as orchestrator with extracted `ConfigStrip`, `StatsBar`, and `ProjectCard` components
- Projects sorted by running-first, then last active

**Library Management**
- Extracted Library page into modular components (PersonaDetail, SkillDetail, McpTab, RepoManager)
- Custom skill, agent, and workflow creation from UI

### v0.2.2 - Themes, Remote Repos, README

- 6 color themes (Obsidian, Cyber, Ember, Daylight, Paper, Lavender)
- Remote skill repository browser (GitHub API)
- README rewrite and documentation

### v0.2.1 - Settings, Library, Skill Scanning

- Settings tabs UI with provider management
- Library page with persona/skill/workflow listing
- Skill scanning and updater signing

### v0.2.0 - MCP, Auto-Update, Provider Detection

- MCP server integration
- Auto-update checker
- Provider auto-detection
- Agent memory support

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

Areas where help is needed:

- **New personas & skills** - expand the built-in library
- **MCP server integrations** - add more preset MCP configurations
- **UI/UX improvements** - better data visualization for agent activity
- **Internationalization** - add more languages

### Development Setup

```bash
git clone https://github.com/KuaaMU/omnihive.git
cd omnihive/app

npm install
npm run tauri dev

# Build for production
npm run tauri build
```

**Requirements:** Node.js 18+, Rust 1.70+, system dependencies for [Tauri 2](https://v2.tauri.app/start/prerequisites/).

## Roadmap

- [x] Tauri desktop app with React UI
- [x] Multi-engine support (Claude Code, Codex, OpenCode)
- [x] Auto-detect existing API configurations
- [x] 6 color themes (dark + light)
- [x] MCP server integration
- [x] Remote skill repository browser
- [x] macOS & Linux builds (CI/CD)
- [x] Interactive agent topology visualization
- [x] Per-project runtime configuration
- [x] Configurable API format (Standard / Claude Code / OpenAI)
- [x] SSE streaming support
- [x] Agent performance analytics panel
- [x] Activity event feed
- [ ] Cost tracking & budget visualization
- [ ] Plugin system for custom engines
- [ ] Skill marketplace (community-driven)
- [ ] Multi-project parallel execution

## Star History

<a href="https://star-history.com/#KuaaMU/omnihive&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=KuaaMU/omnihive&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=KuaaMU/omnihive&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=KuaaMU/omnihive&type=Date" />
 </picture>
</a>

## License

[MIT](LICENSE)

---

<p align="center">
  <sub>Built with Tauri, React, and a lot of autonomous agents.</sub><br/>
  <sub>If this project is useful to you, please consider giving it a <a href="https://github.com/KuaaMU/omnihive">star</a>.</sub>
</p>
