//! Deterministic software image rendering and an optional native three-d window adapter.
use crate::{
    Camera, Color, PreparedSeries, Projection, RenderError, Result, Tetraplot, ViewportError,
};
#[cfg(feature = "image-export")]
use std::path::Path;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}
impl RenderedImage {
    pub const fn width(&self) -> u32 {
        self.width
    }
    pub const fn height(&self) -> u32 {
        self.height
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
    #[cfg(feature = "image-export")]
    pub fn save_png(&self, path: impl AsRef<Path>) -> std::result::Result<(), RenderError> {
        image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| RenderError::ImageEncoding {
                message: "RGBA buffer length did not match dimensions".to_owned(),
            })?
            .save(path)
            .map_err(|error| RenderError::ImageEncoding {
                message: error.to_string(),
            })
    }
}
#[derive(Clone, Copy)]
struct ScreenPoint {
    x: f64,
    y: f64,
    depth: f64,
}
impl Tetraplot {
    pub fn render(&self, dimensions: (u32, u32)) -> Result<RenderedImage> {
        render_plot(self, dimensions)
    }
    #[cfg(feature = "image-export")]
    pub fn save_png(&self, path: impl AsRef<Path>, dimensions: (u32, u32)) -> Result<()> {
        self.render(dimensions)?.save_png(path)?;
        Ok(())
    }
    #[cfg(feature = "window")]
    pub fn show(&self) -> Result<()> {
        three_d_backend::show(self)
    }
    #[cfg(not(feature = "window"))]
    pub fn show(&self) -> Result<()> {
        Err(RenderError::WindowFeatureDisabled.into())
    }
}
fn render_plot(plot: &Tetraplot, dimensions: (u32, u32)) -> Result<RenderedImage> {
    let (width, height) = dimensions;
    if width == 0 || height == 0 {
        return Err(ViewportError::InvalidImageDimensions { width, height }.into());
    }
    let background = plot.background().clamped();
    let mut image = RenderedImage {
        width,
        height,
        rgba: vec![0; width as usize * height as usize * 4],
    };
    for pixel in image.rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&rgba(background));
    }
    let mut depth = vec![f64::INFINITY; width as usize * height as usize];
    let camera = plot.camera();
    let geometry = plot.geometry();
    let frame = plot.frame();
    if frame.faces() {
        for face in crate::Face::ALL {
            draw_triangle(
                &mut image,
                &mut depth,
                geometry
                    .face(face)
                    .map(|index| project(camera, geometry.vertices()[index], width, height)),
                frame.face_color(),
            );
        }
    }
    for section in plot.sections() {
        if !section.visible() {
            continue;
        }
        let Ok(embedding) = section.embedding() else {
            continue;
        };
        if let crate::SectionIntersection::Triangle(triangle) = section.intersection() {
            let fill = Color::rgb(0.35, 0.55, 0.85).with_alpha(section.style().fill_opacity);
            draw_triangle(
                &mut image,
                &mut depth,
                triangle
                    .vertices
                    .map(|vertex| project(camera, vertex.world, width, height)),
                fill,
            );
            for index in 0..3 {
                draw_line(
                    &mut image,
                    &mut depth,
                    project(camera, triangle.vertices[index].world, width, height),
                    project(
                        camera,
                        triangle.vertices[(index + 1) % 3].world,
                        width,
                        height,
                    ),
                    crate::BLACK,
                    1.5,
                );
            }
        }
        for series in section.chart().series() {
            match series {
                crate::DiagramSeries::Points { points, .. } => {
                    for local in points {
                        if let Ok(point) = embedding.map(*local, plot.tolerance()) {
                            draw_disc(
                                &mut image,
                                &mut depth,
                                project(camera, geometry.to_world(point), width, height),
                                crate::RED,
                                7.0,
                            );
                        }
                    }
                }
                crate::DiagramSeries::Line { points, closed, .. } => {
                    for pair in points.windows(2) {
                        if let (Ok(a), Ok(b)) = (
                            embedding.map(pair[0], plot.tolerance()),
                            embedding.map(pair[1], plot.tolerance()),
                        ) {
                            draw_line(
                                &mut image,
                                &mut depth,
                                project(camera, geometry.to_world(a), width, height),
                                project(camera, geometry.to_world(b), width, height),
                                crate::BLUE,
                                2.0,
                            );
                        }
                    }
                    if *closed && points.len() > 2 {
                        if let (Ok(a), Ok(b)) = (
                            embedding.map(*points.last().unwrap(), plot.tolerance()),
                            embedding.map(points[0], plot.tolerance()),
                        ) {
                            draw_line(
                                &mut image,
                                &mut depth,
                                project(camera, geometry.to_world(a), width, height),
                                project(camera, geometry.to_world(b), width, height),
                                crate::BLUE,
                                2.0,
                            );
                        }
                    }
                }
            }
        }
    }
    for (_, series) in plot.prepared_series() {
        match series {
            PreparedSeries::Surface(surface) => {
                for triangle in &surface.triangles {
                    draw_triangle(
                        &mut image,
                        &mut depth,
                        triangle.map(|index| {
                            project(
                                camera,
                                surface.world_vertices[index as usize].map(f64::from),
                                width,
                                height,
                            )
                        }),
                        surface.style.color(),
                    );
                }
            }
            PreparedSeries::Line(line) => {
                for path in &line.paths {
                    for pair in path.windows(2) {
                        draw_line(
                            &mut image,
                            &mut depth,
                            project(camera, pair[0].world.map(f64::from), width, height),
                            project(camera, pair[1].world.map(f64::from), width, height),
                            line.style.color(),
                            line.style.width(),
                        );
                    }
                }
            }
            PreparedSeries::Points { points, style } => {
                for point in points {
                    draw_disc(
                        &mut image,
                        &mut depth,
                        project(camera, point.world.map(f64::from), width, height),
                        style.color(),
                        style.size(),
                    );
                }
            }
        }
    }
    for edge in crate::Edge::ALL {
        let [a, b] = geometry.edge(edge);
        draw_line(
            &mut image,
            &mut depth,
            project(camera, a, width, height),
            project(camera, b, width, height),
            frame.edge_color(),
            frame.edge_width(),
        );
    }
    Ok(image)
}
fn project(camera: Camera, point: [f64; 3], width: u32, height: u32) -> Option<ScreenPoint> {
    let forward = unit(sub(camera.target(), camera.position()))?;
    let right = unit(cross(forward, camera.up()))?;
    let up = cross(right, forward);
    let relative = sub(point, camera.position());
    let z = dot(relative, forward);
    if z < camera.near() || z > camera.far() {
        return None;
    }
    let x = dot(relative, right);
    let y = dot(relative, up);
    let aspect = f64::from(width) / f64::from(height);
    let (nx, ny) = match camera.projection() {
        Projection::Perspective {
            vertical_fov_radians,
        } => {
            let tan = (vertical_fov_radians * 0.5).tan();
            (x / (z * tan * aspect), y / (z * tan))
        }
        Projection::Orthographic { vertical_span } => (
            x / (vertical_span * 0.5 * aspect),
            y / (vertical_span * 0.5),
        ),
    };
    Some(ScreenPoint {
        x: (nx + 1.0) * 0.5 * (f64::from(width) - 1.0),
        y: (1.0 - (ny + 1.0) * 0.5) * (f64::from(height) - 1.0),
        depth: z,
    })
}
fn draw_triangle(
    image: &mut RenderedImage,
    depth: &mut [f64],
    vertices: [Option<ScreenPoint>; 3],
    color: Color,
) {
    let [Some(a), Some(b), Some(c)] = vertices else {
        return;
    };
    let area = edge(a, b, c.x, c.y);
    if area.abs() < f64::EPSILON {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min(f64::from(image.width - 1)) as u32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
    let max_y =
        a.y.max(b.y)
            .max(c.y)
            .ceil()
            .min(f64::from(image.height - 1)) as u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = f64::from(x) + 0.5;
            let py = f64::from(y) + 0.5;
            let w0 = edge(b, c, px, py) / area;
            let w1 = edge(c, a, px, py) / area;
            let w2 = edge(a, b, px, py) / area;
            if w0 >= -1.0e-10 && w1 >= -1.0e-10 && w2 >= -1.0e-10 {
                put(
                    image,
                    depth,
                    x,
                    y,
                    w0 * a.depth + w1 * b.depth + w2 * c.depth,
                    color,
                );
            }
        }
    }
}
fn draw_line(
    image: &mut RenderedImage,
    depth: &mut [f64],
    start: Option<ScreenPoint>,
    end: Option<ScreenPoint>,
    color: Color,
    width: f32,
) {
    let (Some(start), Some(end)) = (start, end) else {
        return;
    };
    let steps = (end.x - start.x)
        .abs()
        .max((end.y - start.y).abs())
        .ceil()
        .max(1.0) as u32;
    for step in 0..=steps {
        let t = f64::from(step) / f64::from(steps);
        draw_disc(
            image,
            depth,
            Some(ScreenPoint {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
                depth: start.depth + (end.depth - start.depth) * t,
            }),
            color,
            width,
        );
    }
}
fn draw_disc(
    image: &mut RenderedImage,
    depth: &mut [f64],
    point: Option<ScreenPoint>,
    color: Color,
    size: f32,
) {
    let Some(point) = point else {
        return;
    };
    let radius = (size.max(1.0) * 0.5).ceil() as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let x = point.x.round() as i32 + dx;
                let y = point.y.round() as i32 + dy;
                if x >= 0 && y >= 0 && x < image.width as i32 && y < image.height as i32 {
                    put(image, depth, x as u32, y as u32, point.depth, color);
                }
            }
        }
    }
}
fn put(image: &mut RenderedImage, depth: &mut [f64], x: u32, y: u32, z: f64, color: Color) {
    let index = y as usize * image.width as usize + x as usize;
    if z > depth[index] + 1.0e-9 {
        return;
    }
    let color = color.clamped();
    if color.alpha() > 0.999 {
        depth[index] = z;
    }
    let offset = index * 4;
    for channel in 0..3 {
        let source = [color.red(), color.green(), color.blue()][channel];
        let destination = f32::from(image.rgba[offset + channel]) / 255.0;
        image.rgba[offset + channel] =
            ((source * color.alpha() + destination * (1.0 - color.alpha())) * 255.0).round() as u8;
    }
    image.rgba[offset + 3] = 255;
}
fn rgba(color: Color) -> [u8; 4] {
    let color = color.clamped();
    [
        (color.red() * 255.0).round() as u8,
        (color.green() * 255.0).round() as u8,
        (color.blue() * 255.0).round() as u8,
        (color.alpha() * 255.0).round() as u8,
    ]
}
fn edge(a: ScreenPoint, b: ScreenPoint, x: f64, y: f64) -> f64 {
    (x - a.x) * (b.y - a.y) - (y - a.y) * (b.x - a.x)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(value: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(value, value).sqrt();
    (length > f64::EPSILON).then_some(value.map(|part| part / length))
}
#[cfg(feature = "window")]
mod three_d_backend {
    use super::*;
    pub fn show(plot: &Tetraplot) -> Result<()> {
        let window = three_d::Window::new(three_d::WindowSettings {
            title: plot.caption().unwrap_or("tetraplot").to_owned(),
            initial_size: Some((1100, 800)),
            ..Default::default()
        })
        .map_err(|error| RenderError::Backend {
            message: error.to_string(),
        })?;
        let context = window.gl();
        let models = models(plot, &context);
        let source = plot.camera();
        let fov = match source.projection() {
            Projection::Perspective {
                vertical_fov_radians,
            } => vertical_fov_radians.to_degrees() as f32,
            Projection::Orthographic { .. } => 45.0,
        };
        let mut camera = three_d::Camera::new_perspective(
            window.viewport(),
            vector(source.position().map(|value| value as f32)),
            vector(source.target().map(|value| value as f32)),
            vector(source.up().map(|value| value as f32)),
            three_d::degrees(fov),
            source.near() as f32,
            source.far() as f32,
        );
        let mut control = three_d::OrbitControl::new(camera.target(), 0.01, source.far() as f32);
        let background = plot.background().clamped();
        window.render_loop(move |mut frame| {
            camera.set_viewport(frame.viewport);
            control.handle_events(&mut camera, &mut frame.events);
            frame
                .screen()
                .clear(three_d::ClearState::color_and_depth(
                    background.red(),
                    background.green(),
                    background.blue(),
                    1.0,
                    1.0,
                ))
                .render(&camera, &models, &[]);
            three_d::FrameOutput::default()
        });
        Ok(())
    }
    fn models(
        plot: &Tetraplot,
        context: &three_d::Context,
    ) -> Vec<three_d::Gm<three_d::Mesh, three_d::ColorMaterial>> {
        let mut result = Vec::new();
        for (_, series) in plot.prepared_series() {
            match series {
                PreparedSeries::Surface(surface) => {
                    let mesh = three_d::CpuMesh {
                        positions: three_d::Positions::F32(
                            surface.world_vertices.iter().copied().map(vector).collect(),
                        ),
                        indices: three_d::Indices::U32(
                            surface.triangles.iter().flatten().copied().collect(),
                        ),
                        ..Default::default()
                    };
                    result.push(model(context, mesh, surface.style.color()));
                }
                PreparedSeries::Line(line) => {
                    let mut positions = Vec::new();
                    let mut indices = Vec::new();
                    for path in &line.paths {
                        for pair in path.windows(2) {
                            tube(
                                &mut positions,
                                &mut indices,
                                pair[0].world,
                                pair[1].world,
                                (line.style.width() * 0.007).max(0.003),
                            );
                        }
                    }
                    if !positions.is_empty() {
                        result.push(model(
                            context,
                            three_d::CpuMesh {
                                positions: three_d::Positions::F32(positions),
                                indices: three_d::Indices::U32(indices),
                                ..Default::default()
                            },
                            line.style.color(),
                        ));
                    }
                }
                PreparedSeries::Points { points, style } => {
                    let mut positions = Vec::new();
                    let mut indices = Vec::new();
                    for point in points {
                        octahedron(
                            &mut positions,
                            &mut indices,
                            point.world,
                            (style.size() * 0.012).max(0.015),
                        );
                    }
                    if !positions.is_empty() {
                        result.push(model(
                            context,
                            three_d::CpuMesh {
                                positions: three_d::Positions::F32(positions),
                                indices: three_d::Indices::U32(indices),
                                ..Default::default()
                            },
                            style.color(),
                        ));
                    }
                }
            }
        }
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for edge in crate::Edge::ALL {
            let [a, b] = plot.geometry().edge(edge);
            tube(
                &mut positions,
                &mut indices,
                a.map(|value| value as f32),
                b.map(|value| value as f32),
                (plot.frame().edge_width() * 0.007).max(0.004),
            );
        }
        result.push(model(
            context,
            three_d::CpuMesh {
                positions: three_d::Positions::F32(positions),
                indices: three_d::Indices::U32(indices),
                ..Default::default()
            },
            plot.frame().edge_color(),
        ));
        result
    }
    fn model(
        context: &three_d::Context,
        mesh: three_d::CpuMesh,
        color: Color,
    ) -> three_d::Gm<three_d::Mesh, three_d::ColorMaterial> {
        let color = color.clamped();
        three_d::Gm::new(
            three_d::Mesh::new(context, &mesh),
            three_d::ColorMaterial {
                color: three_d::Srgba::new(
                    (color.red() * 255.0).round() as u8,
                    (color.green() * 255.0).round() as u8,
                    (color.blue() * 255.0).round() as u8,
                    (color.alpha() * 255.0).round() as u8,
                ),
                is_transparent: color.alpha() < 0.999,
                ..Default::default()
            },
        )
    }
    fn vector(value: [f32; 3]) -> three_d::Vec3 {
        three_d::vec3(value[0], value[1], value[2])
    }
    fn octahedron(
        positions: &mut Vec<three_d::Vec3>,
        indices: &mut Vec<u32>,
        center: [f32; 3],
        radius: f32,
    ) {
        let base = positions.len() as u32;
        for offset in [
            [radius, 0.0, 0.0],
            [-radius, 0.0, 0.0],
            [0.0, radius, 0.0],
            [0.0, -radius, 0.0],
            [0.0, 0.0, radius],
            [0.0, 0.0, -radius],
        ] {
            positions.push(three_d::vec3(
                center[0] + offset[0],
                center[1] + offset[1],
                center[2] + offset[2],
            ));
        }
        for triangle in [
            [0, 2, 4],
            [2, 1, 4],
            [1, 3, 4],
            [3, 0, 4],
            [2, 0, 5],
            [1, 2, 5],
            [3, 1, 5],
            [0, 3, 5],
        ] {
            indices.extend(triangle.map(|value| base + value));
        }
    }
    fn tube(
        positions: &mut Vec<three_d::Vec3>,
        indices: &mut Vec<u32>,
        start: [f32; 3],
        end: [f32; 3],
        radius: f32,
    ) {
        let axis = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
        let length = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if length <= f32::EPSILON {
            return;
        }
        let axis = [axis[0] / length, axis[1] / length, axis[2] / length];
        let reference = if axis[0].abs() < 0.8 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let first = unit3(cross3(axis, reference));
        let second = cross3(axis, first);
        let base = positions.len() as u32;
        for point in [start, end] {
            for index in 0..6 {
                let angle = index as f32 * std::f32::consts::TAU / 6.0;
                positions.push(three_d::vec3(
                    point[0] + radius * (first[0] * angle.cos() + second[0] * angle.sin()),
                    point[1] + radius * (first[1] * angle.cos() + second[1] * angle.sin()),
                    point[2] + radius * (first[2] * angle.cos() + second[2] * angle.sin()),
                ));
            }
        }
        for index in 0..6 {
            let next = (index + 1) % 6;
            indices.extend([
                base + index,
                base + next,
                base + 6 + next,
                base + index,
                base + 6 + next,
                base + 6 + index,
            ]);
        }
    }
    fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    fn unit3(value: [f32; 3]) -> [f32; 3] {
        let length = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
        [value[0] / length, value[1] / length, value[2] / length]
    }
}
