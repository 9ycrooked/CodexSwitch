# Codex Account Switcher Design System

## 1. Visual Theme & Atmosphere

Codex 账号切换器采用 `Cli-Proxy-API-Management-Center` 的暗色管理台气质：暖灰、低饱和、轻玻璃层、数据密度适中。界面应像本地凭据控制台，而不是营销页面。

## 2. Color Palette & Roles

```css
:root {
  --bg-secondary: #151412;
  --bg-primary: #1d1b18;
  --bg-tertiary: #262320;
  --bg-hover: #2e2a26;
  --bg-quinary: #191714;
  --floating-surface: #2a2723;
  --floating-border: #4a443d;
  --text-primary: #f6f4f1;
  --text-secondary: #c9c3bb;
  --text-tertiary: #9c958d;
  --text-quaternary: #6f6962;
  --border-color: #3a3530;
  --border-primary: #4a453f;
  --border-hover: #5a544d;
  --primary-color: #8b8680;
  --primary-hover: #9a948e;
  --primary-active: #a6a099;
  --success-color: #10b981;
  --quota-medium-color: #ffd862;
  --warning-color: #c65746;
  --danger-color: #c65746;
}
```

RGB helpers: primary `139 134 128`, success `16 185 129`, warning/danger `198 87 70`.

## 3. Typography Rules

Use system UI fonts for a desktop-native feel:

```css
font-family: "Segoe UI", "Microsoft YaHei", system-ui, -apple-system, BlinkMacSystemFont, sans-serif;
```

Headings use 700 weight, body text uses 400-500, labels use 600-700. Body text minimum is 14px; Chinese explanatory copy should keep line-height >= 1.65.

## 4. Component Stylings

Buttons use `.btn` semantics mapped to native `button`: `primary`, `secondary`, `ghost`, `danger`. All buttons need hover, active, focus-visible and disabled states. Cards use 10-12px radius, translucent `--bg-primary`, 1px `--border-color`, and mild hover lift.

Inputs/selects use `--bg-secondary`, `--border-color`, and focus ring based on `--primary-color`.

Status badges use compact 6px radius, 10-12px font, and semantic color pairs for active, warning, disabled, and virtual/muted states.

## 5. Layout Principles

Desktop layout is a fixed left sidebar plus flexible content. Main content spacing uses 8px rhythm: 8 / 12 / 16 / 24 / 32. Account cards use responsive grid with minimum width around 360-420px and collapse to one column on mobile.

## 6. Depth & Elevation

Primary cards: `0 1px 3px rgb(0 0 0 / 0.3)`. Floating or selected states may use `0 10px 15px -3px rgb(0 0 0 / 0.3)` plus a subtle primary outline. Avoid heavy glows.

## 7. Animation & Interaction

Interaction level: L2.

Use CSS-only transitions and keyframes:
- `fade-in-up` for cards, backup rows, and settings groups.
- 150-300ms hover/focus transitions.
- No new dependencies.
- `prefers-reduced-motion: reduce` disables transform and entry animations.

## 8. Do's and Don'ts

Do keep density high but readable. Do use semantic variables instead of raw colors in components. Do keep card actions compact. Do preserve keyboard focus rings. Do use inline SVG for icons.

Don't introduce marketing hero sections. Don't use emoji as icons. Don't add fake quota percentages. Don't use purple/green neon as the dominant palette. Don't add new frontend dependencies for this styling pass. Don't hide OAuth/storage warnings.

## 9. Responsive Behavior

At tablet width, sidebar stacks above content and navigation becomes a three-column rail. At mobile width, action buttons wrap, cards become single column, and quota/meta rows stack. All interactive controls must remain at least 44px tall.
