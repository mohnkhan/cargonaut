# Contract: Theme System

Defines the themable elements, the two built-in palettes, and degrade behavior. Satisfies FR-001..FR-007.

## Themable elements (every one MUST be colored — no element left to terminal default)

panel bg/fg · directory · executable · symlink · hidden · cursor row (bg+fg) · marked entry · marked+cursor · focused border · unfocused border · menu bar (bg/fg/selected) · function-key bar (number/label) · status bar (bg/fg) · dialog (bg/fg/selected).

## Built-in: `commander-dark` (default)

Evokes the reference manager's signature look. Indicative values (final hex/index in implementation):

| Element | Color |
|---------|-------|
| panel bg | blue |
| panel fg | light gray / white |
| directory | bright white (bold) |
| executable | bright green |
| symlink | bright cyan |
| hidden | dim gray |
| cursor row | black fg on cyan bg |
| marked | bright yellow |
| marked+cursor | bright yellow on cyan |
| focused border | bright cyan |
| unfocused border | gray |
| menu bar | black on cyan; selected: white on blue |
| fkey bar | label light-gray on black; number black on cyan |
| status bar | black on cyan |
| dialog | black on light-gray; selected: white on blue |

## Built-in: `monochrome` (fallback / 8-color safe)

Uses only the 8 base colors + bold/reverse so it is legible on the most limited terminals. Selection via reverse; directories via bold. Guarantees FR-007 on minimal terminals.

## Resolution & degrade

- `resolve(name)`: exact match → that built-in; unknown → default (`commander-dark`) + emit a non-fatal status notice (FR-006). Never panics.
- `--theme NAME` (CLI) overrides `config.ui.theme` which overrides the compiled default (FR-005).
- Color depth: themes are authored with named/indexed colors for the default so 16-color terminals render correctly; richer themes may use `Rgb` and rely on terminal downsampling (FR-007). The app never fails to render due to color depth.

## Invariants (testable)

- T-THEME-1: every themable element returns a concrete `Color` (no `Option`/none-path at render).
- T-THEME-2: `resolve("does-not-exist")` returns the default theme and a notice.
- T-THEME-3: directory / executable / symlink / regular / hidden rows produce distinct styles in the default theme (SC-002).
- T-THEME-4: cursor row and marked row are mutually distinct and distinct from normal rows (SC-002).
