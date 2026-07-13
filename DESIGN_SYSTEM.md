# Design System — Kryptos UI

## Design Philosophy

This is a mission-control interface for a privacy/surveillance-countermeasure tool. The aesthetic is inspired by SOC consoles, Bloomberg Terminals, and military C2 displays — not consumer security apps. Every visual element must communicate precision, restraint, and authority.

## Color Tokens

### Neutral Palette

| Token | Hex | Usage |
|-------|-----|-------|
| `bg-default` | `#0f1117` | Main page background |
| `bg-surface` | `#1a1d27` | Card/panel background |
| `bg-elevated` | `#232738` | Hovered/active surface |
| `bg-input` | `#161922` | Input field background |
| `border-default` | `#2a2e3d` | Default borders |
| `border-subtle` | `#1f2332` | Divider lines |
| `text-primary` | `#e2e8f0` | Primary body text |
| `text-secondary` | `#94a3b8` | Secondary/label text |
| `text-muted` | `#64748b` | Placeholder/disabled |

### Accent (single, deliberate)

| Token | Hex | Usage |
|-------|-----|-------|
| `accent` | `#5eead4` | Primary accent — active state, toggles, links, highlights |
| `accent-hover` | `#2dd4bf` | Hover state for accent elements |
| `accent-dim` | `#115e59` | Accent at 30% opacity for subtle highlights |

### Status Colors (desaturated)

| Token | Hex | Usage |
|-------|-----|-------|
| `status-ok` | `#6ee7b7` | Running, active, connected |
| `status-warn` | `#fbbf24` | Degraded, restarting, partial |
| `status-critical` | `#f87171` | Failed, error, leak detected |
| `status-info` | `#67e8f9` | Informational, processing |

### Semantic Surface Tones

| Token | Hex | Usage |
|-------|-----|-------|
| `surface-ok` | `#064e3b` | Card background for OK state |
| `surface-warn` | `#451a03` | Card background for warning state |
| `surface-critical` | `#450a0a` | Card background for critical state |

## Typography

### Font Stack
- **Data/Monospace**: `"JetBrains Mono", "Fira Code", "Cascadia Code", "Consolas", monospace`
  - For: IPs, ports, service names, status values, PIDs, timestamps
- **UI/Sans**: `"Inter", "SF Pro", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`
  - For: Labels, body text, headings, navigation

### Type Scale

| Level | Size | Weight | Font Family | Usage |
|-------|------|--------|-------------|-------|
| `display` | 24px / 1.5rem | 600 (semibold) | UI Sans | Page titles |
| `heading` | 16px / 1rem | 600 (semibold) | UI Sans | Section/panel headers |
| `body` | 13px / 0.8125rem | 400 (regular) | UI Sans | General content |
| `data` | 13px / 0.8125rem | 400 (regular) | Monospace | Status values, metrics |
| `label` | 11px / 0.6875rem | 500 (medium) | UI Sans | Field labels, column headers |
| `caption` | 11px / 0.6875rem | 400 (regular) | UI Sans | Metadata, timestamps |
| `small-data` | 11px / 0.6875rem | 400 (regular) | Monospace | Dense data displays |

## Spacing Scale

| Token | Value |
|-------|-------|
| `space-1` | 2px |
| `space-2` | 4px |
| `space-3` | 8px |
| `space-4` | 12px |
| `space-5` | 16px |
| `space-6` | 20px |
| `space-7` | 24px |
| `space-8` | 32px |
| `space-9` | 40px |
| `space-10` | 48px |

## Layout

- **Status bar**: 40px fixed height across top, persistent on all pages
- **Sidebar**: 200px, fixed width, contains navigation only
- **Content area**: Fluid, with 20px padding
- **Max content width**: 1280px
- **Grid**: 12-column, 16px gutter

## Iconography

- Stroke width: 1.5px consistent across all icons
- Use `lucide-react` for the base set
- Preferred icons: `Activity`, `Radio`, `Network`, `Gauge`, `Terminal`, `Route`, `GitBranch`, `Split`, `Shield`, `CircleDot`, `Signal`, `Wifi`, `Server`, `Database`, `Box`, `Layers`

## Motion

- Transitions: 150ms ease-out for micro-interactions
- Data updates: No animation for polling — just in-place value changes
- Loading: Minimal — single thin bar at top of content area, or subtle pulse on skeleton
- State transitions: 300ms ease for anything that communicates a state change
- No decorative animation, no parallax, no rotation effects

## Component Patterns

### Cards
- Background: `bg-surface`, border: `border-default` 1px, border-radius: 8px
- Padding: `space-5` (16px) inside cards
- Header: `heading` weight, `text-primary`, with optional icon at `text-muted`
- Cards never have shadows — use borders for separation

### Buttons
- Primary: accent background, white text, 8px radius
- Secondary: transparent, border only, text-secondary
- Critical: status-critical text, surface-critical background on hover
- No icon backgrounds — keep buttons flat

### Status Badges
- `label` weight, uppercase, with 4px dot indicator
- Background: `bg-elevated`, dot color matches status

### Data Tables / Lists
- Row height: 36px
- Alternating row backgrounds not used — use subtle borders between items
- Font: `data` (monospace) for values, `label` for headers
