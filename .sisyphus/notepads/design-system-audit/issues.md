# Design System Issues

## Critical Issues

### 1. Font Stack Not Implemented

**Location**: `src/index.css` line 61
**Expected**: Cormorant Garamond (headings), Nunito Sans (body) per CLAUDE.md
**Actual**: System font stack
**Impact**: No distinctive typography, looks generic

### 2. Color Palette Mismatch

**Location**: `src/index.css` lines 16-47
**Expected**: ivory (#FFFDF5), espresso (#3D3629), bronze (#8B7355) per CLAUDE.md
**Actual**: Pure white (#FFFFFF), Notion gray (#37352F), gray accent (#9B9A97)
**Impact**: Completely different visual identity than documented

### 3. CSS Variable Conflict

**Location**: `src/index.css`

- Line 13: `--radius: 0.75rem` in `@theme`
- Line 141: `--radius: 0.5rem` in `:root`
  **Impact**: Unpredictable border radius values

### 4. Hardcoded Colors in Badge Component

**Location**: `src/components/ui/badge.tsx` lines 18-24

```tsx
blue: 'bg-[#E8F0FE] text-[#5383EC] dark:bg-[#5383EC]/20 dark:text-[#5383EC]';
```

**Impact**: Breaks design system consistency, harder to maintain

## Moderate Issues

### 5. Status Colors Not Tokenized

**Location**:

- `src/components/FileCard.tsx` lines 16-19
- `src/components/FileList.tsx` lines 61-65
  **Code**:

```tsx
const statusColor = {
  available: 'bg-green-500',
  missing: 'bg-destructive',
  moved: 'bg-yellow-500',
};
```

**Impact**: Inconsistent with design system, no semantic meaning

### 6. No Spacing Scale Defined

**Location**: Missing from `src/index.css`
**Impact**: Components use arbitrary Tailwind values (p-4, gap-2, etc.) without guidance

### 7. Inconsistent Border Radius Usage

**Components**:

- Button: `rounded-md`
- Card: `rounded-lg`
- Badge: `rounded`
- Dialog: `rounded-lg`
- Input: `rounded-md`
  **Impact**: Visual inconsistency

## Minor Issues

### 8. Unused CSS Classes

**Location**: `src/index.css` lines 235-247
**Classes**: `.notion-card-hover`, `.notion-button`, `.notion-gradient-text`
**Impact**: Dead code, potential confusion

### 9. Missing Typography Scale

**Location**: Missing from `src/index.css`
**Impact**: No consistent heading sizes defined as tokens

### 10. No Shadow Scale

**Location**: Missing from `src/index.css`
**Impact**: Components use arbitrary shadow values
