# Design System — Kryptos UI (Intel Agency Aesthetic)

## Design Philosophy

Classified operations console inspired by SOC displays, Bloomberg terminals, and military C2 systems. Monochrome + phosphor green accent. Data density prioritized over whitespace. No rounded chrome.

## Color Palette

### Core
| Token | Hex | Usage |
|-------|-----|-------|
| `black` | `#0a0b0e` | Main background |
| `dark` | `#111318` | Status bar, sidebar, surface |
| `surface` | `#181b24` | Card background |
| `raised` | `#1f2330` | Hover/active surface |
| `border` | `#2a2f3f` | Borders and dividers |
| `text` | `#c8ccd4` | Primary text |
| `text-dim` | `#6b7280` | Secondary/label text |

### Accent
| Token | Hex | Usage |
|-------|-----|-------|
| `green` | `#4ade80` | Phosphor green — active, ok, links |
| `green-dim` | `#166534` | Subtle green backgrounds |
| `amber` | `#f59e0b` | Warnings, degraded |
| `red` | `#ef4444` | Critical, errors, kill switch |
| `cyan` | `#22d3ee` | Informational |

## Typography

- **UI**: `"Inter", "SF Pro", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`
- **Data/Monospace**: `"JetBrains Mono", "Fira Code", "Cascadia Code", "Consolas", monospace`

### Scale
| Level | Size | Weight | Family | Usage |
|-------|------|--------|--------|-------|
| Page title | 15px | 600 | mono | Page headings |
| Card title | 11px | 600 | mono uppercase | Section headers |
| Data | 11px | 400 | mono | Row values |
| Label | 9px | 400 | mono uppercase | Column headers, labels |
| Meta | 8px | 400 | mono | Footer, timestamps |

## Layout

- **Classification banner**: 24px fixed, top of every page. "TOP SECRET // ENDPOINT PRIVACY // KRYPTOS"
- **Status bar**: 32px, below classification. Shows daemon status + service count
- **Sidebar**: 160px, fixed. Left side, navigation only
- **Content**: Fluid, 16px padding. No max-width constraints

## Icons

- lucide-react, stroke-width 1.5px
- Used sparingly — prefer text-based indicators
- Icon color inherits from parent text color

## Component Patterns

### Cards
- Background: `#181b24`, border: `#2a2f3f` 1px, no border-radius
- Padding: 12px
- Header: bottom border divider, card-title style
- No shadows — use borders for separation

### Buttons
- No border-radius. All buttons are flat.
- Primary: green border, green text, transparent background
- Critical: red border, red text
- Ghost: transparent, no border, text-dim
- Uppercase, 10px font-mono, tracking-wider

### Status Indicators
- 6px solid squares (not circles)
- Colors: green (ok), amber (warn), red (critical), cyan (info), border (off)

### Data Tables
- Full width, no alternating rows
- Header: 9px mono uppercase, text-dim, border-bottom
- Cells: 11px mono, text color
- Row hover: subtle raised background

### Process Table
- 7 columns: indicator, name, status, uptime, pid, restarts, actions
- Status values: RUNNING (green), FAILED (red), STARTING/RESTARTING (amber)

## Motion
- 100ms ease transitions for micro-interactions
- No decorative animation
- Blinking cursor effect (1s step-end) for terminal elements
- Optional scan-line overlay via CSS pseudo-element

## Copy Style
- All-caps labels and headers
- Technical, precise language
- No marketing fluff
- OPSEC terminology preferred
