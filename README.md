# mpl-typst-core

High-performance native Rust core extension for Matplotlib Typst rendering backends (PyO3 + ABI3).

## Features

- **Microsecond In-Memory Text Measurement**: Measures exact width, height, baseline, and descent directly through Typst's native layout engine in memory (~30 µs per measurement), eliminating temporary files, subprocess spawning, and anchor-query hacks.
- **Embedded Document Exporter**: Integrated document compiler and rasterizer/vectorizer supporting PDF, PNG (via tiny-skia), and SVG export without requiring `typst-py` or system `typst` CLI.
- **TeX Math Conversion**: Built-in MiTeX translation layer transparently converts LaTeX math equations into native Typst math syntax.
- **System & Custom Font Discovery**: Lazily loads and caches system fonts and custom font search directories using `typst-kit`.
- **Python Stable ABI (ABI3)**: Built with `abi3-py310`, ensuring forward compatibility across Python 3.10, 3.11, 3.12, 3.13, and 3.14+.

## Installation

```bash
pip install mpl-typst-core
```

Or build from source using [maturin](https://github.com/PyO3/maturin):

```bash
maturin build --release
```

## Usage

```python
import mpl_typst_core

measurer = mpl_typst_core.TypstCoreMeasurer(include_system_fonts=True)

# 1. Fast text measurement: returns (width_pt, height_pt, descent_pt)
w, h, d = measurer.measure_text(
    "Sample text",
    font_family=["Times New Roman", "SimSun"],
    font_size_pt=7.0,
)

# 2. Mixed prose and math, exactly as Matplotlib emits it
w, h, d = measurer.measure_text("Lattice Parameter $a$ (nm)", is_math=True)

# 3. Bare LaTeX math source
w, h, d = measurer.measure_text(r"\lambda_B / \text{nm}", is_math=True)

# 4. Document export
measurer.compile_pdf(typst_source, "output.pdf")
measurer.compile_png(typst_source, "output.png", ppi=300.0)
measurer.compile_svg(typst_source, "output.svg")
```

### PDF output is untagged by default

Typst writes a tagged PDF unless `tagged=False`: an accessibility structure
tree (`/StructTreeRoot`, a `/StructElem` per text span and per formula, and a
parent-tree array) plus marked-content operators inside the content streams.
A figure is embedded as an image by whatever document consumes it, so none of
that ever reaches a reader — it is pure overhead, emitted as many small
*uncompressed* objects with one xref entry each.

Measured on a 6.5×4.2 in figure with 400 math+CJK labels: 243.5 KB tagged
(842 objects) vs 68.4 KB untagged (27 objects); the structure tree alone is 830
objects taking 135.0 KB, identical on Linux and Windows. The cost tracks the
number of text runs, so a text-dense figure drops ~72% while an ordinary
line-plot figure drops only ~3% (103.0 KB → 100.0 KB).

Pass `tagged=True` to `render_pdf` / `compile_pdf` for a standalone document
that should be accessible on its own.

### `is_math` semantics

`is_math` mirrors Matplotlib's `RendererBase.get_text_width_height_descent`
contract, so plain text never leaks into the math parser:

| `is_math` | Meaning |
| --- | --- |
| `False` (default) | The whole string is literal text; `$` is escaped for Typst. |
| `True` | Prose interleaved with `$...$` math spans; only those spans are converted. A string without any `$` is treated as bare math source. |
| `"TeX"` | As `True`, except a string without `$` stays literal — Matplotlib probes renderers with `"lp"` under `text.usetex`. |

Inline math is emitted tightly (`$x$`), because Typst treats the
whitespace-padded form (`$ x $`) as display math and lays it out as a block.

### `top_edge` / `bottom_edge`

Both parameters accept a Typst length (`1em`, `12pt`, `2mm`) or an edge metric
(`cap-height`, `ascender`, `x-height`, `baseline`, `bounds`, `descender`).
Lengths are emitted verbatim; metric names are quoted, because an unquoted
metric name is parsed as a variable reference and fails with
"unknown variable".

## License

MIT License.
