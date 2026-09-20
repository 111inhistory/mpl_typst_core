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
    is_math=False
)

# 2. LaTeX math equation measurement
w, h, d = measurer.measure_text(
    r"\lambda_B / \text{nm}",
    font_size_pt=7.0,
    is_math=True
)

# 3. Document export
measurer.compile_pdf(typst_source, "output.pdf")
measurer.compile_png(typst_source, "output.png", ppi=300.0)
measurer.compile_svg(typst_source, "output.svg")
```

## License

MIT License.
