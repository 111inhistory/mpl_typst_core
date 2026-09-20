use std::path::PathBuf;

use comemo::Track;
use parking_lot::RwLock;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use typst::Library;
use typst::LibraryExt;
use typst::World;
use typst::diag::{FileError, FileResult, SourceResult};
use typst::engine::{Engine, Route, Sink, Traced};
use typst::foundations::{Bytes, Datetime, Duration, StyleChain};
use typst::introspection::{EmptyIntrospector, Locator};
use typst::layout::{Abs, Axes, Frame, Region};
use typst::syntax::{FileId, Source};
use typst::text::{Font, FontBook};
use typst::utils::{LazyHash, Protected};
use typst_kit::fonts::FontStore;

struct MeasurerWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    source: RwLock<Source>,
}

impl MeasurerWorld {
    fn new(extra_font_paths: &[PathBuf], include_system_fonts: bool) -> Self {
        let mut fonts = FontStore::new();
        fonts.extend(typst_kit::fonts::embedded());
        if include_system_fonts {
            fonts.extend(typst_kit::fonts::system());
        }
        for path in extra_font_paths {
            fonts.extend(typst_kit::fonts::scan(path));
        }

        let default_source = Source::detached("");
        Self {
            library: LazyHash::new(Library::default()),
            fonts,
            source: RwLock::new(default_source),
        }
    }

    fn set_source(&self, source: Source) {
        *self.source.write() = source;
    }
}

impl World for MeasurerWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.source.read().id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let current = self.source.read();
        if id == current.id() {
            Ok(current.clone())
        } else {
            Err(FileError::NotFound(id.vpath().get_without_slash().into()))
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        Err(FileError::NotFound(id.vpath().get_without_slash().into()))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
    pub descent: f64,
}

impl MeasurerWorld {
    fn measure_source(&self, source: Source) -> SourceResult<TextMetrics> {
        self.set_source(source.clone());

        let world_dyn: &dyn World = self;
        let mut sink = Sink::new();
        let traced = Traced::default();
        let route = Route::default();

        let module = typst_eval::eval(
            world_dyn.track(),
            &self.library,
            traced.track(),
            sink.track_mut(),
            route.track(),
            &source,
        )?;
        let content = module.content();

        let introspector = EmptyIntrospector;
        let mut engine = Engine {
            library: &self.library,
            world: world_dyn.track(),
            introspector: Protected::new(introspector.track()),
            traced: traced.track(),
            sink: sink.track_mut(),
            route: Route::default(),
        };

        let locator = Locator::root();
        let styles = StyleChain::new(&self.library.styles);
        let pod = Region::new(Axes::splat(Abs::inf()), Axes::splat(false));

        let frame: Frame =
            (engine.library.routines.layout_frame)(&mut engine, &content, locator, styles, pod)?;

        let width = frame.width().to_pt();
        let height = frame.height().to_pt();
        let baseline = frame.baseline().to_pt();
        let descent = (height - baseline).max(0.0);

        Ok(TextMetrics {
            width,
            height,
            descent,
        })
    }
}

#[pyclass]
pub struct TypstCoreMeasurer {
    world: MeasurerWorld,
}

const MITEX_PRELUDE: &str = "\
#let textmath(it) = text(it)\n\
#let mitexcolor(c, it) = text(fill: rgb(c), it)\n\
#let mitexoverbrace(it) = overbrace(it)\n\
#let mitexunderbrace(it) = underbrace(it)\n\
";

fn convert_latex_math(s: &str) -> String {
    let trimmed = s.trim();
    let inner = if trimmed.starts_with('$') && trimmed.ends_with('$') && trimmed.len() >= 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };
    if inner.contains('\\') {
        if let Ok(converted) = mitex::convert_math(inner, None) {
            return format!("$ {} $", converted);
        }
    }
    format!("$ {} $", inner)
}

fn prepare_typst_body(text: &str, is_math: bool) -> String {
    if is_math {
        convert_latex_math(text)
    } else if text.contains('$') {
        let mut result = String::with_capacity(text.len() + 16);
        let mut in_math = false;
        let mut math_buf = String::new();

        for ch in text.chars() {
            if ch == '$' {
                if in_math {
                    result.push_str(&convert_latex_math(&math_buf));
                    math_buf.clear();
                    in_math = false;
                } else {
                    in_math = true;
                }
            } else if in_math {
                math_buf.push(ch);
            } else {
                result.push(ch);
            }
        }
        if in_math {
            result.push_str(&convert_latex_math(&math_buf));
        }
        result
    } else {
        text.to_string()
    }
}

#[pymethods]
impl TypstCoreMeasurer {
    #[new]
    #[pyo3(signature = (extra_font_paths=None, include_system_fonts=true))]
    fn new(extra_font_paths: Option<Vec<String>>, include_system_fonts: bool) -> Self {
        let paths: Vec<PathBuf> = extra_font_paths
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let world = MeasurerWorld::new(&paths, include_system_fonts);
        Self { world }
    }

    /// Measures an arbitrary piece of Typst markup.
    ///
    /// Returns (width_pt, height_pt, descent_pt).
    fn measure_source(&self, source_code: &str) -> PyResult<(f64, f64, f64)> {
        let source = Source::detached(source_code);
        let metrics = self
            .world
            .measure_source(source)
            .map_err(|errs| PyRuntimeError::new_err(format!("Typst error: {errs:?}")))?;
        Ok((metrics.width, metrics.height, metrics.descent))
    }

    /// Helper tailored for Matplotlib text elements.
    ///
    /// Constructs styled Typst source with the given fonts, size, and body.
    #[pyo3(signature = (
        text,
        font_family=None,
        font_size_pt=7.0,
        is_math=false,
        top_edge="cap-height",
        bottom_edge="descender",
        par_leading_em=0.65,
        par_spacing_em=1.2
    ))]
    fn measure_text(
        &self,
        text: &str,
        font_family: Option<Vec<String>>,
        font_size_pt: f64,
        is_math: bool,
        top_edge: &str,
        bottom_edge: &str,
        par_leading_em: f64,
        par_spacing_em: f64,
    ) -> PyResult<(f64, f64, f64)> {
        let fonts_typst = match font_family {
            Some(fonts) if !fonts.is_empty() => {
                let list = fonts
                    .iter()
                    .map(|f| format!("\"{}\"", f.replace('\"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({list})")
            }
            _ => "(\"Times New Roman\", \"SimSun\")".to_string(),
        };

        let body = prepare_typst_body(text, is_math);

        let typst_code = format!(
            "{MITEX_PRELUDE}\
             #set par(leading: {par_leading_em}em, spacing: {par_spacing_em}em)\n\
             #set text(font: {fonts_typst}, size: {font_size_pt}pt, top-edge: \"{top_edge}\", bottom-edge: \"{bottom_edge}\")\n\
             {body}\n"
        );

        self.measure_source(&typst_code)
    }

    /// Batch measurement for multiple items in a single FFI call.
    #[pyo3(signature = (
        items,
        font_family=None,
        font_size_pt=7.0,
        is_math=false,
        top_edge="cap-height",
        bottom_edge="descender"
    ))]
    fn measure_batch(
        &self,
        items: Vec<String>,
        font_family: Option<Vec<String>>,
        font_size_pt: f64,
        is_math: bool,
        top_edge: &str,
        bottom_edge: &str,
    ) -> PyResult<Vec<(f64, f64, f64)>> {
        let mut results = Vec::with_capacity(items.len());
        for text in items {
            let res = self.measure_text(
                &text,
                font_family.clone(),
                font_size_pt,
                is_math,
                top_edge,
                bottom_edge,
                0.65,
                1.2,
            )?;
            results.push(res);
        }
        Ok(results)
    }
}

#[pymodule]
fn mpl_typst_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<TypstCoreMeasurer>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_measure_text() {
        let world = MeasurerWorld::new(&[], false);
        let src = Source::detached(
            "#set text(size: 10pt)\n\
             Hello World",
        );
        let metrics = world.measure_source(src).expect("should measure text");
        println!(
            "Width: {}, Height: {}, Descent: {}",
            metrics.width, metrics.height, metrics.descent
        );
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
    }

    #[test]
    fn test_native_measure_math() {
        let world = MeasurerWorld::new(&[], false);
        let src = Source::detached(
            "#set text(size: 10pt)\n\
             $E = m c^2$",
        );
        let metrics = world.measure_source(src).expect("should measure math");
        println!(
            "Math Width: {}, Height: {}, Descent: {}",
            metrics.width, metrics.height, metrics.descent
        );
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
    }
    #[test]
    fn test_native_measure_math_fraction() {
        let world = MeasurerWorld::new(&[], false);
        let src = Source::detached(
            "#set text(size: 10pt)\n\
             $y = (x_1) / (x_2)$",
        );
        let metrics = world.measure_source(src).expect("should measure math fraction");
        println!(
            "Fraction Math Width: {}, Height: {}, Descent: {}",
            metrics.width, metrics.height, metrics.descent
        );
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
        assert!(metrics.descent > 0.0, "fraction denominator must have descent!");
    }
    #[test]
    fn test_native_measure_descender() {
        let world = MeasurerWorld::new(&[], false);
        let src = Source::detached(
            "#set text(size: 10pt, bottom-edge: \"descender\")\n\
             typography with g, j, p, q, y",
        );
        let metrics = world.measure_source(src).expect("should measure descender");
        println!(
            "Descender Width: {}, Height: {}, Descent: {}",
            metrics.width, metrics.height, metrics.descent
        );
        assert!(metrics.width > 0.0);
        assert!(metrics.height > 0.0);
        assert!(metrics.descent > 0.0, "descent must be positive for descenders!");
    }

    #[test]
    fn test_mitex_conversion() {
        let latex = r"\lambda_B / \text{nm}";
        let typst_math = mitex::convert_math(latex, None).expect("convert latex math");
        let world = MeasurerWorld::new(&[], false);
        let src = Source::detached(format!(
            "#let textmath(it) = text(it)\n$ {} $",
            typst_math
        ));
        let metrics = world.measure_source(src).expect("measure mitex math");
        println!("MiTeX Math Width: {}, Height: {}, Descent: {}", metrics.width, metrics.height, metrics.descent);
        assert!(metrics.width > 0.0);
    }
}
