# Design System Decisions

## Architectural Choices

### 1. shadcn/ui as Component Foundation

**Decision**: Use shadcn/ui primitives for all UI components
**Rationale**:

- Accessible by default (Radix UI primitives)
- Customizable via Tailwind
- No runtime dependency (copy-paste approach)
- Consistent API patterns

### 2. Tailwind CSS 4.x

**Decision**: Use Tailwind CSS 4.x with new syntax
**Rationale**:

- Modern `@theme` syntax for design tokens
- `@plugin` syntax instead of postcss.config.js
- Better CSS-first configuration

### 3. CSS Variables for Theming

**Decision**: All colors defined as CSS variables
**Rationale**:

- Enables runtime theme switching
- Supports dark mode via CSS cascade
- Semantic naming (primary, secondary, etc.)

### 4. Notion-Inspired Design

**Decision**: Adopt Notion's visual language
**Rationale**:

- Clean, minimal aesthetic
- Familiar to users
- Good for content-focused applications

### 5. CVA for Variant Management

**Decision**: Use class-variance-authority for component variants
**Rationale**:

- Type-safe variant definitions
- Clean API for consumers
- Consistent with shadcn/ui patterns

## Trade-offs

### 1. System Fonts vs Custom Fonts

**Current**: System font stack
**Trade-off**:

- (+) No font loading delay
- (+) Native feel on each platform
- (-) No distinctive character
- (-) Inconsistent with documentation

### 2. Hardcoded Badge Colors

**Current**: Hex values in Badge variants
**Trade-off**:

- (+) Exact Notion color match
- (+) No token dependency
- (-) Breaks design system
- (-) Harder to maintain

### 3. Multiple Theme Blocks

**Current**: Three `@theme` blocks in index.css
**Trade-off**:

- (+) Separates light/dark themes
- (+) Inline computed values
- (-) Potential conflicts
- (-) Confusing precedence
