# Biblio Design System Audit

## Executive Summary

Biblio uses a **Notion-inspired design system** built on shadcn/ui primitives with Tailwind CSS 4.x. The system is well-structured with semantic color tokens, consistent component patterns, and proper dark mode support. However, there are notable inconsistencies between documented design intent (CLAUDE.md) and actual implementation.

---

## 1. Design Tokens Inventory

### 1.1 Color Palette

**Light Theme (Primary)**
| Token | Value | Usage |
|-------|-------|-------|
| `--color-background` | `#FFFFFF` | Page background |
| `--color-foreground` | `#37352F` | Primary text (Notion-style dark gray) |
| `--color-primary` | `#37352F` | Buttons, emphasis |
| `--color-primary-foreground` | `#FFFFFF` | Text on primary |
| `--color-secondary` | `#F7F6F3` | Hover states, cards |
| `--color-muted` | `#F7F6F3` | Subtle backgrounds |
| `--color-muted-foreground` | `#787774` | Secondary text |
| `--color-accent` | `#9B9A97` | Focus states, highlights |
| `--color-destructive` | `#EB5757` | Errors, delete actions |
| `--color-border` | `#E9E9E7` | Borders, dividers |
| `--color-sidebar` | `#FBFBFA` | Sidebar background |

**Notion-Style Accent Colors**
| Token | Value | Usage |
|-------|-------|-------|
| `--color-notion-blue` | `#5383EC` | Blue badges, selection |
| `--color-notion-purple` | `#9065B0` | Purple badges |
| `--color-notion-pink` | `#E255A1` | Pink badges |
| `--color-notion-red` | `#EB5757` | Red badges (same as destructive) |
| `--color-notion-orange` | `#D9730D` | Orange badges |
| `--color-notion-yellow` | `#DFAB01` | Yellow badges |
| `--color-notion-green` | `#4DAB9A` | Green badges |
| `--color-notion-gray` | `#9B9A97` | Gray badges |

**Dark Theme**
| Token | Value |
|-------|-------|
| `--color-background` | `#191919` |
| `--color-foreground` | `#E6E6E5` |
| `--color-card` | `#252525` |
| `--color-primary` | `#FFFFFF` |
| `--color-secondary` | `#2F2F2F` |
| `--color-border` | `#373737` |

### 1.2 Border Radius

| Token          | Value                              | Usage           |
| -------------- | ---------------------------------- | --------------- |
| `--radius`     | `0.75rem` (12px)                   | Base radius     |
| `--radius-sm`  | `calc(var(--radius) - 4px)` = 8px  | Small elements  |
| `--radius-md`  | `calc(var(--radius) - 2px)` = 10px | Medium elements |
| `--radius-lg`  | `var(--radius)` = 12px             | Large elements  |
| `--radius-xl`  | `calc(var(--radius) + 4px)` = 16px | Extra large     |
| `--radius-2xl` | `calc(var(--radius) + 8px)` = 20px | Cards, modals   |

**Note:** There's a conflict - `@theme` sets `--radius: 0.75rem` but `:root` sets `--radius: 0.5rem`. The `@theme inline` block references `var(--radius)` which would use the `:root` value.

### 1.3 Typography

**Font Stack (from index.css)**

```css
font-family:
  -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif, 'Apple Color Emoji',
  'Segoe UI Emoji';
```

**Heading Styles**

- `font-weight: 600`
- `letter-spacing: -0.02em`
- `line-height: 1.3`

**Body Styles**

- `font-weight: 400`
- `line-height: 1.5`

**Font Sizes (from components)**

- `text-xs` - Badges, labels, small text
- `text-sm` - Body text, inputs, buttons
- `text-lg` - Dialog titles

### 1.4 Spacing Patterns

**Component Spacing**

- `p-4` (16px) - Card content, dialog padding
- `p-6` (24px) - Dialog content
- `px-3 py-1.5` - Button default padding
- `gap-2` (8px) - Common gap between elements
- `gap-4` (16px) - Section gaps
- `space-y-1.5` - Vertical spacing in headers

### 1.5 Shadows

- `shadow-sm` - Card hover
- `shadow-md` - Dropdowns, popovers
- `shadow-lg` - Dialogs

### 1.6 Transitions

- `duration-100` - Button background transitions
- `duration-200` - Card hover, general transitions
- `transition-colors` - Color changes
- `transition-opacity` - Opacity changes
- `transition-transform` - Transform changes

---

## 2. Component Styling Patterns

### 2.1 shadcn/ui Primitives (17 components)

All components follow consistent patterns:

1. **Radix UI primitives** as base
2. **`cn()` utility** for class merging
3. **CSS variables** for theming
4. **CVA (class-variance-authority)** for variants (Button, Badge)
5. **Forward refs** for all components

### 2.2 Button Variants

```typescript
variant: {
  default: "bg-primary text-primary-foreground hover:bg-primary/90",
  destructive: "bg-destructive text-destructive-foreground hover:bg-destructive/90",
  outline: "border border-input bg-transparent hover:bg-secondary",
  secondary: "bg-secondary text-secondary-foreground hover:bg-secondary/80",
  ghost: "hover:bg-secondary text-foreground",
  link: "text-primary underline-offset-4 hover:underline",
}

size: {
  default: "h-9 px-3 py-1.5",
  sm: "h-8 px-2.5 py-1 text-xs",
  lg: "h-10 px-4 py-2",
  icon: "h-9 w-9",
}
```

### 2.3 Badge Variants

```typescript
variant: {
  default: "bg-primary/10 text-primary",
  secondary: "bg-secondary text-secondary-foreground",
  destructive: "bg-destructive/10 text-destructive",
  outline: "border border-input text-foreground",
  // Notion-style colors with hardcoded hex values
  blue: "bg-[#E8F0FE] text-[#5383EC] dark:bg-[#5383EC]/20 dark:text-[#5383EC]",
  purple: "bg-[#F3E8FC] text-[#9065B0] dark:bg-[#9065B0]/20 dark:text-[#9065B0]",
  // ... etc
}
```

### 2.4 Animation Patterns

All animated components use `tailwindcss-animate`:

```css
data-[state=open]:animate-in data-[state=closed]:animate-out
data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0
data-[state=closed]:zoom-out-98 data-[state=open]:zoom-in-98
```

---

## 3. Tailwind CSS 4.x Usage

### 3.1 Correct Usage

- `@import 'tailwindcss'` - Modern import syntax
- `@plugin 'tailwindcss-animate'` - Plugin syntax (correct for v4)
- `@theme` blocks for design tokens
- `@custom-variant dark` for dark mode
- `@layer base` for base styles

### 3.2 Theme Configuration

Three theme blocks:

1. `@theme { ... }` - Light theme defaults
2. `@media (prefers-color-scheme: dark) { @theme { ... } }` - Dark theme
3. `@theme inline { ... }` - Computed radius values

---

## 4. Visual Consistency Analysis

### 4.1 Consistent Patterns (Good)

1. **Semantic color tokens** - All components use CSS variables
2. **Focus states** - Consistent `focus-visible:ring-2 focus-visible:ring-ring`
3. **Border radius** - Consistent `rounded-md` for inputs, `rounded-lg` for cards
4. **Typography scale** - Consistent use of `text-sm` for body
5. **Transition timing** - Consistent `duration-100` to `duration-200`
6. **Disabled states** - Consistent `disabled:opacity-50`

### 4.2 Inconsistencies Found

#### Critical Issues

1. **Font Stack Mismatch**
   - **CLAUDE.md specifies**: Cormorant Garamond (headings), Nunito Sans (body)
   - **Actual implementation**: System font stack (-apple-system, BlinkMacSystemFont, etc.)
   - **Impact**: No distinctive typography character

2. **Color Palette Mismatch**
   - **CLAUDE.md specifies**: ivory background (#FFFDF5), espresso brown primary (#3D3629), bronze accent (#8B7355)
   - **Actual implementation**: Pure white (#FFFFFF), Notion gray (#37352F), gray accent (#9B9A97)
   - **Impact**: Completely different visual identity

3. **Radius Conflict**
   - `@theme` sets `--radius: 0.75rem`
   - `:root` sets `--radius: 0.5rem`
   - `@theme inline` uses `var(--radius)` which resolves to `:root` value
   - **Impact**: Inconsistent border radius calculations

4. **Hardcoded Colors in Badge**
   - Badge variants use hardcoded hex values instead of design tokens
   - Example: `bg-[#E8F0FE] text-[#5383EC]`
   - **Impact**: Breaks design system consistency

#### Moderate Issues

5. **Status Colors Not Tokenized**
   - FileCard uses hardcoded colors: `bg-green-500`, `bg-yellow-500`
   - FileList uses hardcoded colors: `text-green-600`, `text-red-600`, `text-yellow-600`
   - **Impact**: Inconsistent with design system

6. **Inconsistent Border Radius Usage**
   - Button: `rounded-md`
   - Card: `rounded-lg`
   - Badge: `rounded` (no size specified)
   - Dialog: `rounded-lg`
   - **Impact**: Visual inconsistency

7. **Missing Spacing Scale**
   - No defined spacing scale in design tokens
   - Components use arbitrary Tailwind values
   - **Impact**: Inconsistent spacing

---

## 5. Animation & Transition Patterns

### 5.1 Defined Animations

1. **Notion Card Hover** (custom class)

   ```css
   .notion-card-hover {
     transition:
       box-shadow 200ms ease 0s,
       transform 200ms ease 0s;
   }
   .notion-card-hover:hover {
     box-shadow:
       rgba(15, 15, 15, 0.1) 0px 0px 0px 1px,
       rgba(15, 15, 15, 0.1) 0px 2px 4px;
     transform: translateY(-1px);
   }
   ```

2. **Notion Button** (custom class)

   ```css
   .notion-button {
     transition: background 20ms ease-in 0s;
   }
   ```

3. **Gradient Text** (utility class)
   ```css
   .notion-gradient-text {
     background: linear-gradient(90deg, #5383ec 0%, #9065b0 50%, #e255a1 100%);
     -webkit-background-clip: text;
     -webkit-text-fill-color: transparent;
   }
   ```

### 5.2 Component Animations

- **Dialog/AlertDialog**: fade + zoom (98% scale)
- **Dropdown**: fade + zoom (95% scale) + slide
- **Tooltip**: fade + zoom (95% scale) + slide
- **Select**: fade + zoom (98% scale)

### 5.3 Unused Animation Classes

The custom `.notion-card-hover` and `.notion-button` classes are defined but **not used** in any component. Components use inline Tailwind transitions instead.

---

## 6. Recommendations

### High Priority

1. **Resolve Documentation vs Implementation Gap**
   - Either update CLAUDE.md to match current Notion-style implementation
   - Or implement the documented warm color palette

2. **Fix Radius Conflict**
   - Remove duplicate `--radius` definition in `:root`
   - Keep single source of truth in `@theme`

3. **Tokenize Badge Colors**
   - Replace hardcoded hex values with CSS variables
   - Create semantic tokens for Notion accent colors

### Medium Priority

4. **Create Spacing Scale**
   - Define consistent spacing tokens
   - Document usage guidelines

5. **Tokenize Status Colors**
   - Create semantic tokens: `--color-success`, `--color-warning`, `--color-error`
   - Replace hardcoded green/yellow/red in components

6. **Standardize Border Radius**
   - Document when to use each radius size
   - Ensure consistent application

### Low Priority

7. **Remove Unused CSS Classes**
   - `.notion-card-hover` and `.notion-button` are defined but unused
   - Either use them or remove them

8. **Add Typography Scale**
   - Define heading sizes as tokens
   - Create consistent typography utilities

---

## 7. Component Inventory

### UI Primitives (src/components/ui/)

- alert-dialog.tsx
- badge.tsx
- button.tsx
- card.tsx
- dialog.tsx
- dropdown-menu.tsx
- input.tsx
- label.tsx
- popover.tsx
- progress.tsx
- scroll-area.tsx
- select.tsx
- separator.tsx
- switch.tsx
- table.tsx
- tabs.tsx
- tooltip.tsx

### Application Components (src/components/)

- AuthorBadge.tsx
- AuthorManager.tsx
- CategoryDetailPage.tsx
- CategoryManager.tsx
- CategorySelect.tsx
- CategorySidebar.tsx
- DeleteConfirmDialog.tsx
- DropZone.tsx
- DynamicMetadataForm.tsx
- EmptyState.tsx
- ErrorBoundary.tsx
- FileCard.tsx
- FileEditDialog.tsx
- FileList.tsx
- FilePicker.tsx
- FilterPanel.tsx
- ImportProgress.tsx
- LoadingState.tsx
- MetadataEditor.tsx
- MetadataField.tsx
- SearchBar.tsx
- SearchResults.tsx
- SettingsDialog.tsx
- StoragePathSetting.tsx
- TagBadge.tsx
- TagInput.tsx
- TagManager.tsx
