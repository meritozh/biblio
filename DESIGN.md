# Biblio design

## Character

Biblio is a quiet working catalogue for a personal collection. The files are the content and the chrome stays out of their way: one paper surface, one ink, one accent, single-pixel rules. Remove a colour, border or box before adding one. Give the collection title and the files themselves the strongest typographic presence and keep search, import and maintenance tools in plain sight.

The app icon's ruled slip and index tab survive as a monochrome line mark: an outlined slip with a small filled tab in the accent colour. There is no gold, no coloured navigation cloth and no multicoloured cards.

## Colour

Use the semantic colours from the UI crate. Every surface is a neutral; the accent is the only chromatic colour and is reserved for the principal action, keyboard focus, selected choices and the index tab. Porcelain is a cool neutral with an ink-blue accent. Graphite is a warm, monochrome neutral whose accent is the ink itself. Both follow the saved light, dark or system appearance.

| Role | Porcelain light | Porcelain dark | Graphite light | Graphite dark |
| --- | --- | --- | --- | --- |
| Paper (workspace) | `#FAFAF8` | `#16181C` | `#F6F4F0` | `#1C1B19` |
| Rail (sidebar, compact bar) | `#F3F3F0` | `#1B1D22` | `#EFECE6` | `#21201D` |
| Sheet (fields, slips) | `#FFFFFF` | `#1F2227` | `#FFFDF9` | `#272522` |
| Ink | `#1E2229` | `#ECECE8` | `#2A2724` | `#EEEAE3` |
| Secondary ink | `#5E6470` | `#A3A8B2` | `#67615A` | `#B3ACA2` |
| Rule (dividers) | `#E4E4DF` | `#2C3036` | `#E3DED5` | `#35322E` |
| Keyline (control edges) | `#C9CAC4` | `#41464E` | `#CCC5B9` | `#4A4640` |
| Wash (hover, current row) | `#EAEAE6` | `#262A30` | `#E8E4DC` | `#2D2A26` |
| Accent | `#2F4A9E` | `#9DB2F0` | `#2A2724` | `#EEEAE3` |
| On accent | `#FFFFFF` | `#11182E` | `#FFFFFF` | `#1C1B19` |
| Tint (selected choice, drag) | `#E9EDF7` | `#232B3D` | `#EAE5DC` | `#322F2A` |

Confirmed destructive actions use a separate rust-red lettering and keyline. Keep normal text at a contrast ratio of at least 4.5:1 against every surface it sits on, including the rail and the wash.

## Type and space

Set the wordmark, page titles, section headings and coverless book titles in Georgia with a generic serif fallback. Everything else — navigation, inputs, paths, metadata and actions — uses the native sans serif so mixed-script filenames get platform fallback. Page titles are 28 logical pixels, section headings 18, catalogue slip titles 17, controls and body text 14, and navigation group labels and catalogue status 12. Use sentence case everywhere, including group labels and choice captions.

Work on a 4-pixel unit. Use 8 between related controls, 12 within a form and 24 or more between regions. Controls use a 6-pixel corner and catalogue slips a 4-pixel corner. Depth comes from tone and single-pixel rules only: no shadows, no cards nested inside cards.

## Window and navigation

A 220-pixel rail on the left carries the wordmark and the destinations, grouped under Library, Organize and Workspace. The rail is a slightly darker tone of the paper with a hairline on its right edge. Rows are 32 pixels high with a secondary-ink icon and ink label. The current row gets the wash fill, a semibold label and an accent icon, so it never relies on colour alone. The current location stays highlighted while reading a file. Long category lists scroll inside the rail without affecting the page.

Below 760 pixels the rail collapses into a top bar on the same rail tone with the wordmark and a labelled Navigation toggle. Opening it replaces the workspace with the full-width rail; choosing a destination closes it.

## Pages

A page starts with one serif title and a hairline below it. The library's Import files action sits on the title line and wraps below the title when space runs short. Search comes first, then the display tools, then a quiet count line with paging, then results. Pages whose content is a form keep a readable measure of about 680 pixels; the collection grid uses the full width.

Forms group related fields under smaller serif section headings separated from the previous group by a rule and generous space. Labels sit above their fields. Inline choices (mode, palette, category) are a row of choices led by a fixed-width label.

## Collection

Use real cover art where available, cropped to the slip's 3:4 frame. A file without artwork gets a flat white slip with a hairline keyline: its format in small secondary capitals at the top, the actual title in serif in the middle and the reading progress at the bottom. All slips share the same neutral so the shelf reads as one collection. The grid fits as many 156-pixel minimum columns as the window holds — two columns even on a phone-width window — and each slip takes the full column width.

There is no frame around a grid item. Beneath the slip, in order: authors, any card metadata, then a single 12-pixel line with storage and status on the left and the labelled favourite star on the right. A favourite star fills with the accent.

The empty shelf is not a box. It is the line mark, a serif sentence and one line of help, centred in open space. Distinguish an empty library, an empty category and a search with no matches, and name the next action for each.

## Import

The drop area is an open paper region with a dashed keyline, the import glyph, a serif invitation and the two pickers. While files are dragged over it the keyline turns solid accent and the area fills with the tint. The batch status sits on the same line as Analyze selected files and Discard import batch.

## Controls and feedback

Use one filled accent button for the principal action of a task. Secondary buttons are transparent with a keyline and take the wash on hover. Selected choices take the tint fill and an accent keyline. Checkboxes are drawn boxes with a check, never text glyphs. Press feedback is immediate; navigation, typing and filtering do not animate. Every focusable control shows a 2-pixel accent focus outline. Accessible labels are identical in the regular and compact layouts. Status and error messages appear in a single ruled line at the foot of the workspace.
