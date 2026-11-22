# Biblio design

## Character

Biblio is a quiet desktop catalogue for a personal collection. The files are the content and the chrome stays out of their way: one paper surface, one ink, one accent, single-pixel rules. Remove a colour, border or box before adding one. The collection title and the files themselves carry the strongest typography; search, import and maintenance stay one press away.

The app icon's ruled slip and index tab survive as a monochrome line mark: an outlined slip with a small filled tab in the accent colour.

This document follows the NSH design guide (`nsh.dev/docs/design-guides`): native, quiet, precise; composition before invention; tokens before values; platform conventions before web habits; visible state; honest feedback; keyboard and screen-reader access as hard requirements.

## User tasks

1. Find a file: search, filter, sort, open.
2. Add files: drop or pick, choose a category, review the analysis, import.
3. Keep the catalogue tidy: edit a file's title, progress, category, tags, authors, metadata and cover; bulk-assign; move to or from remote storage; remove.
4. Shape the catalogue: categories, schemas, tags, authors, prompts, cleanup tools, settings.

Every screen states its object in the title, keeps its principal action visible, and explains empty, loading and error states in place.

## Tokens

All colours come from `ui::theme`; feature code never writes a literal colour. Porcelain is a cool neutral with an ink-blue accent; Graphite is warm and monochrome (its accent is the ink). Both follow the saved light, dark or system appearance.

| Role | Porcelain light | Porcelain dark | Graphite light | Graphite dark |
| --- | --- | --- | --- | --- |
| `paper` workspace | `#FAFAF8` | `#16181C` | `#F6F4F0` | `#1C1B19` |
| `rail` sidebar, bars | `#F3F3F0` | `#1B1D22` | `#EFECE6` | `#21201D` |
| `sheet` fields, slips, popups | `#FFFFFF` | `#1F2227` | `#FFFDF9` | `#272522` |
| `ink` text | `#1E2229` | `#ECECE8` | `#2A2724` | `#EEEAE3` |
| `secondary` muted text | `#5E6470` | `#A3A8B2` | `#67615A` | `#B3ACA2` |
| `rule` dividers | `#E4E4DF` | `#2C3036` | `#E3DED5` | `#35322E` |
| `keyline` control edges | `#C9CAC4` | `#41464E` | `#CCC5B9` | `#4A4640` |
| `wash` hover, current row | `#EAEAE6` | `#262A30` | `#E8E4DC` | `#2D2A26` |
| `accent` primary, focus, selection | `#2F4A9E` | `#9DB2F0` | `#2A2724` | `#EEEAE3` |
| `on_accent` | `#FFFFFF` | `#11182E` | `#FFFFFF` | `#1C1B19` |
| `tint` selected fill, drag | `#E9EDF7` | `#232B3D` | `#EAE5DC` | `#322F2A` |
| `danger` destructive text | `#9A3B2F` | `#FFB4A8` | same | same |

Normal text keeps at least 4.5:1 against every surface it sits on, including `rail`, `wash` and `tint`.

Radii: `radius` 6 for controls, popups and rows; `radius_sm` 4 for slips, chips and small marks. Spacing follows the 4-point scale (2, 4, 8, 12, 16, 24, 32). Control height is 32 (medium) or 28 (small, in toolbars and rows); icons are 16 in a fixed slot. Depth is tone and hairlines; only popups (menus, popovers, select lists, tooltips) and dialogs get a shadow.

Type: page titles 26 Georgia, section headings 17 Georgia, slip titles 16 Georgia; everything else is the platform sans: body and controls 14, captions and status 12. Sentence case everywhere.

## Component layer

Feature modules compose `crate::ui`; they do not style `nsh_base` controls themselves. The layer, one file per family under `crates/biblio-ui/src/ui/`:

- `theme` — tokens above, motion tokens, `Palette` context.
- `icon` — one 24-unit stroke icon set drawn at 16, the line mark, the favourite star.
- `button` — `Button` with variants `Primary` (the default submit of the area), `Default` (keyline), `Ghost` (toolbars, rows), `Danger`; sizes `Sm` 28 and `Md` 32; optional leading icon; `icon_button` is square, needs an accessible name and shows a tooltip.
- `field` — text input, labelled field, multiline, search field, checkbox (nsh-base checkbox with a drawn indicator).
- `choice` — segmented control (nsh-base `ToggleGroup`), choice chips, `Select` (nsh-base select with the app's trigger and popup).
- `overlay` — popup surface shared by menus, popovers, select lists and tooltips; `menu`, `menu_item`, `menu_radio`, `menu_separator`, `popover`, `tooltip`; `tool_trigger` (a quiet toolbar menu face: muted name, current value, chevron); `modal`, the dialog that creates and edits records; `confirm` shows the native alert dialog for irreversible actions.
- `table` — nsh-base `DataTable` in the tokens: one flexible column, fixed-width columns, narrow icon-command columns (the page handles their presses by column key), a height that fits its rows, and `badge`.
- `layout` — page, page header, page body, section, notice, empty state, disclosure row.

## Window

**Window chrome.** On macOS the system title bar folds into the app (NSH `TitleBarStyle::Hidden`, 52 points): the rail runs to the window's top edge with the window controls in its top band, and the top band of the workspace drags the window. Light and dark therefore reach the whole window, title bar included. The menu bar carries the commands and their shortcuts: Biblio (Settings…), File (Import Files…), Edit (standard items, Find in Library), Go (All Files, Back to Library, Categories, Tags, Authors, Cleanup) and Window.

**Sidebar workspace.** A 232-point rail holds the wordmark (at the foot of the title bar band, past the window controls), the Library group (All files, then each category), the Manage group (Categories, Tags, Authors, Cleanup), a hairline, the Advanced group (Schemas, Prompts) and, pinned to the rail's foot, Settings. Import is the library header's Add button. Rows are 32 high with a 16-point icon slot; the current row has the `tint` fill with an accent label and icon. Long category lists scroll inside the rail. The current location stays highlighted while a file is open.

**Pages.** Every page starts with the same header across the workspace: the page's icon, its serif title, a muted count beside it, and its commands at the trailing edge (search, then the primary action). A hairline divides it from the body. On macOS the header is also the window's drag band. The text reader keeps the book title and reading tools in one drag band. Its scrolling text sits in a centered column, with contents on the left and reading settings on the right when opened. Compact windows present these panels as sheets.

Below 760 points the rail becomes a top bar on the `rail` tone (it is also the window's drag band, inset past the window controls) with the wordmark and a labelled Navigation toggle; opening it shows the full-width rail, and choosing a destination closes it.

**Status bar.** A 28-point bar at the foot of the workspace shows job progress ("Working…") and the latest result or error. It is the only place transient feedback appears, so the page never jumps.

**Keyboard.** `mod+F` and `mod+K` focus search (from any page they open All files first), `mod+I` opens Import, `mod+,` opens Settings, `mod+1` opens All files, `mod+[` returns from a file to the library. The menu bar shows them.

## Pages

### Library

- Header: the collection title and its file count, then the search field, Lucky (three random picks in a dialog, with Shuffle again) and Add (primary, opens Import).
- Toolbar under the header: View (in a category: All, By author, By title), Sort (name, date added, date modified and the schema's sortable fields; the trigger shows the current key), ascending/descending, Favorites only (a toggle for the favourite condition), and Filters (popover, shows the number of active conditions); Grid/List and Select at the trailing edge.
- Search suggestions open as a list surface directly under the field; arrows move a highlighted row, Return or Tab accepts it, Escape closes it. Active filters show as removable chips under the toolbar.
- Selection mode adds a tinted bar under the toolbar: count, Select all shown (the files loaded on the shelf; none while the groups themselves show), Clear, Assign tag, Assign author (popovers with search), Download, and a More menu with Upload to Baidu Netdisk, Clear cached copies and Delete (each confirmed by an alert naming the scope).
- Results scroll on their own; the header and toolbar stay put. There is no pager: the shelf is a virtualized list that makes only what is in view and loads the next 60 files as it nears the last. A change elsewhere (an edit, a deletion) reloads as many files as are shown, so the shelf stays where it was; returning from a file scrolls back to the first file that was in view.
- Grid: as many 156-point minimum columns as fit, 28 apart each way. Each item is a 3:4 slip (real cover, or the title set on a plain ruled slip with format and progress) with the favourite star in its top leading corner (always shown on a favourite, on hover or focus otherwise) and the storage mark (local or remote) in its bottom leading corner; then the title, authors and card metadata. A right-click on a slip opens a context menu (Open, Favorite/Unfavorite, Copy path, Select).
- List: a data table with Title (and the selection box while selecting), Authors, Format, Status, Added and the favourite star. A press on a row opens the file, or toggles it while selecting; the star column toggles the favourite. It loads more as it scrolls, as the grid does.
- Grouped views show the groups as slips (the first file's cover, title, count) or as a table; a press opens a group, and "All groups" above its files returns.
- Empty states are open space with the line mark, one serif sentence and one line naming the next action; empty library, empty category and no matches are distinct.

### File

A two-pane page on wide windows: a 200-point leading pane with the slip or cover, Open file and Favorite; and the form pane at a 640-point measure under the file's path. The page header has Back to library (`mod+[`) before the file's title, its format beside it, and a More actions menu (Copy path, Show in folder, Download or Clear cached copy or Move to Baidu Netdisk, Choose cover, Remove cover, Delete file). Sections: Details (title, progress, category select, Save file), Tags, Authors (chips with remove, plus a search field whose matches add on press), Metadata (one row per value with Save and Remove, and an Add row). On narrow windows the panes stack.

### Import

Titled Add files. Category as a select; an open drop area with a dashed keyline that turns solid accent on a `tint` fill while files are over it; the chosen sources listed beneath with remove buttons; then Analyze selected files (primary), Discard import batch, and the batch status on the same line. The review appears below as a two-pane section: staged cover on the leading side, form on the other, actions at its foot (Import this file is the default submit).

### Forms and settings

Settings uses a tab list (General, Language model, Baidu Netdisk, Database) whose single indicator slides between tabs. Rows pair a label (with a one-line description when the consequence is not obvious) with its control. Immediate choices (appearance mode, palette, import mode) are segmented controls; settings saved by a button use checkboxes. Admin pages put their records in data tables across the workspace, one line of explanation above where the page's purpose is not obvious: Categories (name, description, schema badge, folder, view defaults), Tags and Authors (name with the tag's colour, files; a search field in the header; they load more as they scroll), Schemas (name with a Built-in badge, slug, pipeline, extensions, fields, Export). A row's pencil (or a double press) opens it in a dialog, and the header's primary button opens the same dialog empty; the dialog ends with Cancel and the default submit, and closes on save. A row's bin asks first. Prompts are cards (name, schema and step badge, Active, a line of the text) with Edit and Delete. Cleanup keeps a 680-point measure of setting rows.

## Controls and states

Every control designs rest, hover (`wash`, 120 ms), press (opacity and a 0.97 scale), focus-visible (2-point accent outline outside the control), selected (persistent fill or `tint` with accent keyline, never only colour), disabled (no hover or press) and busy (the status bar). Primary is reserved for the default submit of an area and a page's one creating command: Analyze selected files, Import this file, Save file, a dialog's Create or Save, Add, New category (tag, author, schema, prompt), Save settings. Import, Browse, Test connection and similar commands use the default button. Destructive, irreversible actions (delete a file or record, delete selected files, remove a cover, disconnect remote storage, discard an import batch, delete a schema, tag, author, prompt or preset) open a native alert naming the object and the consequence, with Cancel as the safe default. Icon-only buttons always have an accessible name and a tooltip.

## Motion checklist

Tokens: `instant` 120 ms (hover, colour), `fast` 180 ms (press, control state), `base` 240 ms (moving selection, disclosure), `slow` 320 ms (popup entrance). Entrances ease out, moves ease in-out, exits ease in and run faster than entrances. Press scale 0.97 (0.95 for icon buttons). With reduced motion every transition completes instantly and nothing scales.

| Change | Motion | Reduced motion |
| --- | --- | --- |
| Button, row, nav item hover | background fades to `wash`, `instant`, ease-out | instant |
| Button and icon button press | scale to 0.97 / 0.95 and dim, `fast`, soft-ease | dim only |
| Slip hover and press | slip dims slightly, `instant`; press scales 0.98 | dim only |
| Favourite star toggled | star colour fades, `fast` | instant |
| Sidebar current item | fill fades to the new row, `instant` (rows do not slide: the rail scrolls and categories change) | instant |
| Settings tab change | one indicator slides to the chosen tab, `base`, ease-in-out (native tabs indicator) | jumps |
| Segmented control selection | selected fill fades, `fast` | instant |
| Menus, popovers, select lists | grow from the trigger: fade from 0 and rise 4 points, `slow` ease-out; leave with fade, `fast` ease-in | appear and disappear |
| Dialogs | backdrop and sheet fade in (platform modal, fade); Escape, the backdrop and Close dismiss | appear and disappear |
| Tooltips | fade in after 500 ms rest, `instant` | appear |
| Search suggestion list | appears in place without motion (typing never animates) | — |
| Disclosure rows (groups, records) | chevron rotates 90°, `base`; the content region springs open and closed (nsh-base collapsible reveal, critically damped) | chevron jumps, content appears |
| Drop area drag over | keyline and fill fade, `instant` | instant |
| Page change, filtering, loading more | none: navigation and filtering are immediate so results read as the same place; loaded rows join the end of the list | — |
| Status bar message | text replaces in place; no toast | — |
| Alert dialogs | native platform presentation | platform |

Check hover, press, menu entrance, tab selection, disclosure and drag-over feedback in the native app with both motion preferences.
