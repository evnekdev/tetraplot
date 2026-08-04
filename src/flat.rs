//! Optional Plotters/plotters-ternary adapter for local ternary diagrams.

use std::path::Path;

use plotters::{
    backend::BitMapBackend,
    drawing::IntoDrawingArea,
    prelude::{Color as _, *},
};
use plotters_ternary::{
    PixelPoint, PixelRect, TernaryChartBuilder, TernaryGeometry,
    TernaryPoint as PlottersTernaryPoint, TernaryViewport, ViewportAlignment, ViewportFit,
    ViewportTransform,
};

use crate::{
    BreakLineId, ChartEmbedding, DiagramSeries, EmbeddedChartStyle, FlatViewTarget,
    SectionSeriesId, TernaryDiagram, TernaryPoint, Tetraplot,
};

#[derive(Clone, Debug, PartialEq)]
pub struct FlatChartImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    transform: ViewportTransform,
}
impl FlatChartImage {
    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    pub fn local_at_pixel(&self, pixel: [f32; 2]) -> Option<TernaryPoint> {
        let logical = self
            .transform
            .pixel_to_logical(PixelPoint::new(f64::from(pixel[0]), f64::from(pixel[1])))
            .ok()?;
        let point = TernaryGeometry::default()
            .unproject(logical, plotters_ternary::Tolerance::default())
            .ok()?;
        Some(TernaryPoint::from_unchecked(point.as_array()))
    }
    pub fn pixel_at_local(&self, point: TernaryPoint) -> Option<[f32; 2]> {
        let [a, b, c] = point.as_array();
        let logical = TernaryGeometry::default()
            .project(
                PlottersTernaryPoint::new(a, b, c),
                plotters_ternary::Normalization::RequireUnitSum,
                plotters_ternary::Tolerance::default(),
            )
            .ok()?;
        let pixel = self.transform.logical_to_pixel(logical).ok()?;
        Some([pixel.x as f32, pixel.y as f32])
    }
}

#[derive(Clone, Debug, Default)]
pub struct FlatRenderOptions {
    pub selected_series: Option<SectionSeriesId>,
    pub selected_break: Option<BreakLineId>,
    pub selected_points: Vec<TernaryPoint>,
    pub grid_points: Vec<TernaryPoint>,
    pub linked_cursor: Option<TernaryPoint>,
    pub component_labels: [String; 3],
}
impl FlatRenderOptions {
    pub fn local_defaults() -> Self {
        Self {
            component_labels: ["u".to_owned(), "v".to_owned(), "w".to_owned()],
            ..Self::default()
        }
    }
}

pub struct FlatChart<'a> {
    diagram: &'a TernaryDiagram,
    style: EmbeddedChartStyle,
    embedding: Option<&'a ChartEmbedding>,
    options: FlatRenderOptions,
}
impl<'a> FlatChart<'a> {
    pub fn options_mut(&mut self) -> &mut FlatRenderOptions {
        &mut self.options
    }
    pub fn render(&self, dimensions: (u32, u32)) -> crate::Result<FlatChartImage> {
        let (width, height) = dimensions;
        if width < 96 || height < 96 {
            return Err(crate::ViewportError::InvalidImageDimensions { width, height }.into());
        }
        let mut rgb = vec![255u8; width as usize * height as usize * 3];
        {
            let root = BitMapBackend::with_buffer(&mut rgb, dimensions).into_drawing_area();
            self.draw(root)?;
        }
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for pixel in rgb.chunks_exact(3) {
            rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
        }
        Ok(FlatChartImage {
            width,
            height,
            rgba,
            transform: flat_transform(dimensions)?,
        })
    }
    pub fn save_png(&self, path: impl AsRef<Path>, dimensions: (u32, u32)) -> crate::Result<()> {
        let root = BitMapBackend::new(path.as_ref(), dimensions).into_drawing_area();
        self.draw(root)
    }
    fn draw<DB: DrawingBackend>(
        &self,
        root: DrawingArea<DB, plotters::coord::Shift>,
    ) -> crate::Result<()>
    where
        DB::ErrorType: std::error::Error + Send + Sync,
    {
        root.fill(&WHITE).map_err(render_error)?;
        let mut chart = TernaryChartBuilder::on(&root)
            .margin(36)
            .build()
            .map_err(render_error)?;
        let mut mesh = chart
            .configure_mesh()
            .boundary_style(BLACK.stroke_width(2))
            .corner_a_name(&self.options.component_labels[0])
            .corner_b_name(&self.options.component_labels[1])
            .corner_c_name(&self.options.component_labels[2]);
        if self.style.grid.visible && self.style.grid.subdivisions > 1 {
            mesh = mesh
                .major_step(1.0 / f64::from(self.style.grid.subdivisions))
                .major_grid_style(
                    as_rgb(self.style.grid.color)
                        .mix(f64::from(self.style.grid.color.alpha()))
                        .stroke_width(self.style.grid.width.max(1.0) as u32),
                );
        } else {
            mesh = mesh.hide_grid_lines();
        }
        let mesh = mesh.build();
        mesh.draw_background(&mut chart).map_err(render_error)?;
        for series in self.diagram.series() {
            match series {
                DiagramSeries::Points { id, points } => {
                    let mut style = as_rgb(self.style.point_color).filled();
                    let size = if self.options.selected_series == Some(*id) {
                        9
                    } else {
                        6
                    };
                    if self.options.selected_series == Some(*id) {
                        style = as_rgb(crate::RED).filled();
                    }
                    chart
                        .draw_series(
                            plotters_ternary::TernaryPointSeries::new(
                                points.iter().copied().map(as_plotters_point),
                            )
                            .size(size)
                            .style(style),
                        )
                        .map_err(render_error)?;
                }
                DiagramSeries::Line { id, points, closed } => {
                    let mut values: Vec<_> =
                        points.iter().copied().map(as_plotters_point).collect();
                    if *closed && values.len() > 1 {
                        values.push(values[0]);
                    }
                    let width = if self.options.selected_series == Some(*id) {
                        4
                    } else {
                        2
                    };
                    let color = if self.options.selected_series == Some(*id) {
                        as_rgb(crate::RED)
                    } else {
                        as_rgb(self.style.line_color)
                    };
                    chart
                        .draw_series(plotters_ternary::TernaryLineSeries::new(
                            values,
                            color.stroke_width(width),
                        ))
                        .map_err(render_error)?;
                }
            }
        }
        self.draw_embedding_overlays(&mut chart)?;
        for point in &self.options.grid_points {
            chart
                .draw_series(
                    plotters_ternary::TernaryPointSeries::new([as_plotters_point(*point)])
                        .size(5)
                        .style(BLUE.mix(0.8).filled()),
                )
                .map_err(render_error)?;
        }
        for point in &self.options.selected_points {
            chart
                .draw_series(
                    plotters_ternary::TernaryPointSeries::new([as_plotters_point(*point)])
                        .size(10)
                        .style(MAGENTA.filled()),
                )
                .map_err(render_error)?;
        }
        if let Some(point) = self.options.linked_cursor {
            chart
                .draw_series(
                    plotters_ternary::TernaryPointSeries::new([as_plotters_point(point)])
                        .size(8)
                        .style(RED.filled()),
                )
                .map_err(render_error)?;
        }
        mesh.draw_foreground(&mut chart).map_err(render_error)?;
        mesh.draw_text(&mut chart).map_err(render_error)?;
        root.present().map_err(render_error)?;
        Ok(())
    }
    fn draw_embedding_overlays<'b, DB: DrawingBackend + 'b>(
        &self,
        chart: &mut plotters_ternary::TernaryChart<'b, DB>,
    ) -> crate::Result<()>
    where
        DB::ErrorType: std::error::Error + Send + Sync,
    {
        let Some(ChartEmbedding::Triangulated(embedding)) = self.embedding else {
            return Ok(());
        };
        for triangle in embedding.triangles() {
            let mut line: Vec<_> = triangle
                .map(|index| as_plotters_point(embedding.local_vertices()[index as usize]))
                .to_vec();
            line.push(line[0]);
            chart
                .draw_series(plotters_ternary::TernaryLineSeries::new(
                    line,
                    RGBColor(120, 120, 135).mix(0.55).stroke_width(1),
                ))
                .map_err(render_error)?;
        }
        for break_line in embedding.break_lines() {
            if !break_line.style().visible {
                continue;
            }
            let points: Vec<_> = break_line
                .vertices()
                .iter()
                .map(|index| as_plotters_point(embedding.local_vertices()[index.0 as usize]))
                .collect();
            let selected = self.options.selected_break == Some(break_line.id());
            let color = if selected {
                MAGENTA
            } else {
                as_rgb(break_line.style().color)
            };
            let width = if selected {
                break_line.style().width.max(3.5)
            } else {
                break_line.style().width
            };
            chart
                .draw_series(plotters_ternary::TernaryLineSeries::new(
                    points,
                    color.stroke_width(width.max(1.0) as u32),
                ))
                .map_err(render_error)?;
        }
        Ok(())
    }
}

impl Tetraplot {
    pub fn flat_chart(&self, target: FlatViewTarget) -> crate::Result<FlatChart<'_>> {
        match target {
            FlatViewTarget::Section(id) => {
                let section = self
                    .section(id)
                    .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
                Ok(FlatChart {
                    diagram: section.chart(),
                    style: section.style(),
                    embedding: None,
                    options: FlatRenderOptions::local_defaults(),
                })
            }
            FlatViewTarget::EmbeddedChart(id) => {
                let chart = self
                    .embedded_chart(id)
                    .ok_or(crate::SectionError::UnknownSection { id: id.get() })?;
                Ok(FlatChart {
                    diagram: chart.diagram(),
                    style: chart.style(),
                    embedding: Some(chart.embedding()),
                    options: FlatRenderOptions::local_defaults(),
                })
            }
            FlatViewTarget::None | FlatViewTarget::FollowSelection => {
                Err(crate::RenderError::Backend {
                    message: "flat rendering needs an explicit section or embedded-chart target"
                        .to_owned(),
                }
                .into())
            }
        }
    }
}

fn as_plotters_point(point: TernaryPoint) -> PlottersTernaryPoint {
    let [a, b, c] = point.as_array();
    PlottersTernaryPoint::new(a, b, c)
}
fn as_rgb(color: crate::Color) -> RGBColor {
    RGBColor(
        (color.red().clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.green().clamp(0.0, 1.0) * 255.0).round() as u8,
        (color.blue().clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}
fn render_error(error: impl std::fmt::Display) -> crate::TetraplotError {
    crate::RenderError::Backend {
        message: error.to_string(),
    }
    .into()
}
fn flat_transform((width, height): (u32, u32)) -> crate::Result<ViewportTransform> {
    let margin = 36;
    let inner_width = width
        .checked_sub(margin * 2)
        .ok_or(crate::ViewportError::InvalidImageDimensions { width, height })?;
    let inner_height = height
        .checked_sub(margin * 2)
        .ok_or(crate::ViewportError::InvalidImageDimensions { width, height })?;
    ViewportTransform::new(
        TernaryViewport::full(TernaryGeometry::default()),
        PixelRect::new(margin as i32, margin as i32, inner_width, inner_height)
            .map_err(render_error)?,
        ViewportFit::PreserveAspect,
        ViewportAlignment::Center,
    )
    .map_err(render_error)
}
