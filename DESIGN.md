# Biblio design

## Character

Biblio is a working catalogue for a personal collection. Borrow the paper slips, blue ink and small gold index tab from the existing app icon. Give the collection title and the files themselves the strongest typographic presence. Keep search, import and maintenance tools close at hand.

## Colour

Use semantic colours from the UI crate. Porcelain is the default paper stock. Graphite uses warmer neutrals. Both follow the saved light, dark or system appearance.

| Role | Porcelain light | Porcelain dark | Graphite light | Graphite dark |
| --- | --- | --- | --- | --- |
| Workspace paper | `#F7F6F1` | `#181E2A` | `#F2EEE7` | `#211F1D` |
| Raised sheet and field | `#FFFEFA` | `#232C3B` | `#FFFAF3` | `#2E2B28` |
| Body ink | `#222C40` | `#F2F2EC` | `#302D2A` | `#F4EEE4` |
| Secondary ink | `#586579` | `#B1BDCC` | `#655F55` | `#C5BBAE` |
| Paper rule | `#D0D4D6` | `#414D60` | `#D5CDBF` | `#504A43` |
| Action fill | `#344FB2` | `#ACC1FF` | `#414E8E` | `#BDC7FA` |
| Action lettering | `#FFFFFF` | `#152346` | `#FFFFFF` | `#212748` |
| Navigation cloth | `#23324F` | `#121A29` | `#35343B` | `#19181C` |
| Selected navigation | `#3C5177` | `#304361` | `#54525C` | `#423E47` |

The navigation uses ivory lettering, pale blue-grey group labels and a small gold detail from the app icon. Reserve gold for the index tab. Give confirmed destructive actions a separate rust-red treatment. Keep normal text at a contrast ratio of at least 4.5:1 against its surface.

## Type and space

Set the wordmark and page headings in Georgia with a generic serif fallback. Use the native sans serif for navigation, inputs, paths and actions so mixed-script filenames can use platform fallback. Page headings are 34 logical pixels, section headings 22, controls and body text 14, and group labels 12. Use sentence case for actions and small spaced capitals for navigation groups.

Work on a 4-pixel spacing unit. Use 8 between related controls, 12 within a form, and 24 or more between regions. Use a 6-pixel corner on controls and an 8-pixel corner on catalogue sheets. Use tonal contrast and single-pixel rules for depth. Shadows are unnecessary on paper.

## Window and pages

The full-height cloth sidebar carries the Biblio mark and groups destinations under Library, Organize and Workspace. Keep the active location visible when reading a file. Narrow windows get a labelled navigation toggle above the workspace. Scrolling long category lists must leave the current page usable.

A page starts with one title. A horizontal rule separates its heading from its tools or content. Put the library's import action on the title line when space allows. Search and display tools precede the file count and results. Forms use smaller ruled section headings and a readable measure. Actions wrap when space runs short.

## Collection and feedback

Use real cover art where available. For files without artwork, typeset the actual title on a flat, ruled catalogue slip. Use a small fixed set of paper tints so neighbouring files belong to the same collection. The grid fits as many 200-pixel minimum columns as the window can hold. Each slip uses the available column width. Keep storage status and the labelled favorite star together below the cover. The empty shelf echoes the index tab in the app icon. Distinguish an empty library or category from a search with no matches and explain the relevant next action.

Give the import drop area a paper surface and a visible change of fill and border while files are dragged over it. Place the batch status next to the analysis controls.

Use one filled primary button for the principal action of a task. Secondary controls have a paper face and keyline. Persistent selections use a tinted fill or contrasting navigation row. On hover and press, change the face or opacity immediately. Show a visible focus outline for keyboard users. Keep the same accessible labels in regular and compact layouts. Navigation, typing and filtering do not animate.
