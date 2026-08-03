//! Native editor shell, compiled only with the `editor` feature.

use std::time::Instant;

use three_d::{
    Camera as NativeCamera, ClearState, FrameOutput, GUI, OrbitControl, Window, WindowSettings,
};

use crate::{
    EditorCommand, EditorState, FlatChartImage, FlatViewTarget, GridExportOptions, LinkedCursor,
    LinkedCursorOwner, Selection, Tetraplot, TetraplotDocument,
};

/// Application-style scientific editor. The document remains the sole scientific source of truth.
pub struct TetraplotEditor {
    document: TetraplotDocument,
    state: EditorState,
}
impl TetraplotEditor {
    pub fn new(plot: Tetraplot) -> Self {
        Self {
            document: TetraplotDocument::new(plot),
            state: EditorState::default(),
        }
    }
    pub fn with_document(document: TetraplotDocument) -> Self {
        Self {
            document,
            state: EditorState::default(),
        }
    }
    pub fn document(&self) -> &TetraplotDocument {
        &self.document
    }
    pub fn document_mut(&mut self) -> &mut TetraplotDocument {
        &mut self.document
    }
    pub fn state(&self) -> &EditorState {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut EditorState {
        &mut self.state
    }
    pub fn run(self) -> crate::Result<()> {
        native::run(self)
    }
}
impl Tetraplot {
    pub fn show_editor(self) -> crate::Result<()> {
        TetraplotEditor::new(self).run()
    }
}

mod native {
    use super::*;
    use three_d::egui;

    pub(super) fn run(mut editor: TetraplotEditor) -> crate::Result<()> {
        let window = Window::new(WindowSettings {
            title: editor
                .document
                .plot()
                .caption()
                .unwrap_or("tetraplot editor")
                .to_owned(),
            initial_size: Some((1440, 960)),
            ..Default::default()
        })
        .map_err(backend)?;
        let context = window.gl();
        let mut models = crate::render::three_d_backend::models(editor.document.plot(), &context)?;
        let source = editor.document.plot().camera();
        let fov = match source.projection() {
            crate::Projection::Perspective {
                vertical_fov_radians,
            } => vertical_fov_radians.to_degrees() as f32,
            crate::Projection::Orthographic { .. } => 45.0,
        };
        let mut camera = NativeCamera::new_perspective(
            window.viewport(),
            vec3(source.position().map(|value| value as f32)),
            vec3(source.target().map(|value| value as f32)),
            vec3(source.up().map(|value| value as f32)),
            three_d::degrees(fov),
            source.near() as f32,
            source.far() as f32,
        );
        let mut orbit = OrbitControl::new(camera.target(), 0.01, source.far() as f32);
        let mut gui = GUI::new(&context);
        let mut clipboard = arboard::Clipboard::new().ok();
        let mut model_revision = editor.document.revision();
        let mut flat_cache: Option<FlatTexture> = None;
        let started = Instant::now();
        window.render_loop(move |mut frame| {
            camera.set_viewport(frame.viewport);
            let mut interaction = None;
            gui.update(
                &mut frame.events,
                started.elapsed().as_secs_f64() * 1000.0,
                frame.viewport,
                frame.device_pixel_ratio,
                |ui| {
                    interaction = ui_shell(ui, &mut editor, &mut clipboard, &mut flat_cache);
                },
            );
            if let Some(command) = interaction
                && let Err(error) = command.execute(&mut editor.document, &mut editor.state)
            {
                editor.state.status = Some(error.to_string());
            }
            editor.state.clear_removed(&editor.document);
            if editor.document.revision() != model_revision {
                match crate::render::three_d_backend::models(editor.document.plot(), &context) {
                    Ok(updated) => {
                        models = updated;
                        model_revision = editor.document.revision();
                    }
                    Err(error) => editor.state.status = Some(error.to_string()),
                }
            }
            handle_3d_pick(&frame.events, &camera, &mut editor);
            orbit.handle_events(&mut camera, &mut frame.events);
            let background = editor.document.plot().background().clamped();
            let screen = frame.screen();
            let result = screen
                .clear(ClearState::color_and_depth(
                    background.red(),
                    background.green(),
                    background.blue(),
                    1.0,
                    1.0,
                ))
                .render(&camera, &models, &[])
                .write(|| gui.render());
            if let Err(error) = result {
                editor.state.status = Some(error.to_string());
            }
            FrameOutput::default()
        });
        Ok(())
    }

    struct FlatTexture {
        key: (FlatViewTarget, u64, u64, u32, u32),
        image: FlatChartImage,
        texture: egui::TextureHandle,
    }

    fn ui_shell(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        flat_cache: &mut Option<FlatTexture>,
    ) -> Option<EditorCommand> {
        let mut command = None;
        egui::Panel::top("editor-menu").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("tetraplot editor").strong());
                if ui.button("Fit scene").clicked() {
                    command = Some(EditorCommand::FitCamera)
                }
                if ui.button("Save 3D PNG").clicked() {
                    editor.state.status = editor
                        .document
                        .plot()
                        .save_png("tetraplot-editor-scene.png", (1600, 1200))
                        .err()
                        .map(|error| error.to_string())
                        .or(Some("Saved tetraplot-editor-scene.png".to_owned()));
                }
                if ui.button("Open flat view").clicked() {
                    command = Some(EditorCommand::OpenFlatView(FlatViewTarget::FollowSelection))
                }
                if ui.button("Data").clicked()
                    && let Some(grid) = editor
                        .state
                        .active_grid
                        .or_else(|| editor.document.grids().next().and_then(|grid| grid.id()))
                {
                    command = Some(EditorCommand::OpenDataGrid(grid));
                }
            });
        });
        egui::Panel::left("scene-tree")
            .default_size(225.0)
            .resizable(true)
            .show_inside(ui, |ui| scene_tree(ui, editor, &mut command));
        egui::Panel::right("properties")
            .default_size(250.0)
            .resizable(true)
            .show_inside(ui, |ui| properties(ui, editor, &mut command));
        egui::Panel::bottom("lower-panel")
            .default_size(300.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                lower_panel(ui, editor, clipboard, flat_cache, &mut command)
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("Interactive 3D tetrahedral viewport")
                        .color(egui::Color32::from_gray(225)),
                );
                ui.label(
                    egui::RichText::new(
                        "Orbit: drag  •  Zoom: wheel  •  Select chart objects from the scene tree",
                    )
                    .small()
                    .color(egui::Color32::from_gray(190)),
                );
            });
        });
        egui::Panel::bottom("status")
            .exact_size(23.0)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let text = editor.state.status.as_deref().unwrap_or("Ready");
                    ui.label(text);
                    if let Some(cursor) = &editor.state.linked_cursor {
                        ui.separator();
                        let [u, v, w] = cursor.local_position.as_array();
                        ui.label(format!("local [{u:.4}, {v:.4}, {w:.4}]"));
                        let [a, b, c, d] = cursor.tetrahedral_position.as_array();
                        ui.label(format!("tetra [{a:.4}, {b:.4}, {c:.4}, {d:.4}]"));
                    }
                });
            });
        command
    }

    fn scene_tree(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        command: &mut Option<EditorCommand>,
    ) {
        ui.heading("Scene");
        selectable(
            ui,
            &editor.state.selection,
            Selection::Frame,
            "Tetrahedron frame",
            command,
        );
        egui::CollapsingHeader::new("Planar sections")
            .default_open(true)
            .show(ui, |ui| {
                let sections: Vec<_> = editor
                    .document
                    .plot()
                    .sections()
                    .filter_map(|section| {
                        section.id().map(|id| {
                            (
                                id,
                                section.label().unwrap_or("Section").to_owned(),
                                section.visible(),
                            )
                        })
                    })
                    .collect();
                for (id, name, visible) in sections {
                    ui.horizontal(|ui| {
                        let mut shown = visible;
                        if ui.checkbox(&mut shown, "").changed() {
                            *command = Some(EditorCommand::SetVisibility {
                                target: Selection::Section(id),
                                visible: shown,
                            })
                        }
                        selectable(
                            ui,
                            &editor.state.selection,
                            Selection::Section(id),
                            &name,
                            command,
                        );
                    });
                }
            });
        egui::CollapsingHeader::new("Embedded charts")
            .default_open(true)
            .show(ui, |ui| {
                let charts: Vec<_> = editor
                    .document
                    .plot()
                    .embedded_charts()
                    .filter_map(|chart| {
                        chart.id().map(|id| {
                            (
                                id,
                                chart.visible(),
                                chart
                                    .embedding()
                                    .as_triangulated()
                                    .map(|surface| {
                                        surface
                                            .patches()
                                            .iter()
                                            .map(|patch| {
                                                (
                                                    patch.id(),
                                                    patch.name().unwrap_or("Patch").to_owned(),
                                                )
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default(),
                                chart
                                    .embedding()
                                    .as_triangulated()
                                    .map(|surface| {
                                        surface
                                            .break_lines()
                                            .iter()
                                            .map(|line| line.id())
                                            .collect::<Vec<_>>()
                                    })
                                    .unwrap_or_default(),
                            )
                        })
                    })
                    .collect();
                for (id, visible, patches, breaks) in charts {
                    ui.horizontal(|ui| {
                        let mut shown = visible;
                        if ui.checkbox(&mut shown, "").changed() {
                            *command = Some(EditorCommand::SetVisibility {
                                target: Selection::EmbeddedChart(id),
                                visible: shown,
                            })
                        }
                        selectable(
                            ui,
                            &editor.state.selection,
                            Selection::EmbeddedChart(id),
                            format!("Chart {}", id.get()),
                            command,
                        );
                    });
                    for (patch, name) in patches {
                        ui.indent((id.get(), patch.get()), |ui| {
                            selectable(
                                ui,
                                &editor.state.selection,
                                Selection::SurfacePatch { chart: id, patch },
                                name,
                                command,
                            )
                        });
                    }
                    for line in breaks {
                        ui.indent((id.get(), line.get()), |ui| {
                            selectable(
                                ui,
                                &editor.state.selection,
                                Selection::BreakLine { chart: id, line },
                                format!("Break line {}", line.get()),
                                command,
                            )
                        });
                    }
                }
            });
        egui::CollapsingHeader::new("Data")
            .default_open(true)
            .show(ui, |ui| {
                let grids: Vec<_> = editor
                    .document
                    .grids()
                    .filter_map(|grid| {
                        grid.id()
                            .map(|id| (id, grid.name().to_owned(), grid.rows_len()))
                    })
                    .collect();
                for (id, name, rows) in grids {
                    if ui
                        .selectable_label(
                            editor.state.selection == Selection::Grid(id),
                            format!("{name} ({rows} rows)"),
                        )
                        .clicked()
                    {
                        *command = Some(EditorCommand::OpenDataGrid(id));
                    }
                }
            });
    }
    fn selectable(
        ui: &mut egui::Ui,
        current: &Selection,
        target: Selection,
        label: impl Into<egui::WidgetText>,
        command: &mut Option<EditorCommand>,
    ) {
        if ui.selectable_label(*current == target, label).clicked() {
            *command = Some(EditorCommand::SetSelection(target));
        }
    }

    fn properties(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        command: &mut Option<EditorCommand>,
    ) {
        ui.heading("Properties");
        match editor.state.selection.clone() {
            Selection::None => {
                ui.label("Select a scene object.");
            }
            Selection::Section(id) => {
                if let Some(section) = editor.document.plot().section(id) {
                    ui.label(section.label().unwrap_or("Planar section"));
                    ui.label(format!("{:?}", section.plane()));
                    if ui.button("Open flat view").clicked() {
                        *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::Section(id)))
                    }
                    if ui.button("Remove section").clicked() {
                        editor.document.remove_section(id);
                        editor.state.select(Selection::None);
                    }
                }
            }
            Selection::EmbeddedChart(id) => {
                if let Some(chart) = editor.document.plot().embedded_chart(id) {
                    ui.label(format!("Embedded chart {}", id.get()));
                    let mut opacity = chart.style().fill_opacity;
                    if ui
                        .add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("opacity"))
                        .changed()
                    {
                        *command = Some(EditorCommand::SetOpacity {
                            target: Selection::EmbeddedChart(id),
                            opacity,
                        })
                    }
                    if ui.button("Open flat view").clicked() {
                        *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::EmbeddedChart(
                            id,
                        )))
                    }
                    if ui.button("Remove chart").clicked() {
                        editor.document.remove_embedded_chart(id);
                        editor.state.select(Selection::None);
                    }
                }
            }
            Selection::SurfacePatch { chart, patch } => {
                ui.label(format!("Patch {}", patch.get()));
                ui.label(format!("Chart {}", chart.get()));
            }
            Selection::BreakLine { chart, line } => {
                ui.label(format!("Break line {}", line.get()));
                ui.label(format!("Chart {}", chart.get()));
            }
            Selection::Grid(id) => {
                if let Some(grid) = editor.document.grid(id) {
                    ui.label(grid.name());
                    ui.label(format!("{} rows", grid.rows_len()));
                    if ui.button("Open data table").clicked() {
                        *command = Some(EditorCommand::OpenDataGrid(id));
                    }
                }
            }
            Selection::GridRow { grid, row } => {
                ui.label(format!("Grid {} row {}", grid.get(), row.get()));
            }
            _ => {
                ui.label("Properties are available for this selection.");
            }
        }
    }

    fn lower_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        flat_cache: &mut Option<FlatTexture>,
        command: &mut Option<EditorCommand>,
    ) {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    !matches!(editor.state.flat_view, FlatViewTarget::None),
                    "Flat ternary",
                )
                .clicked()
            {
                *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::FollowSelection));
            }
            if ui
                .selectable_label(editor.state.active_grid.is_some(), "Data table")
                .clicked()
                && let Some(grid) = editor.state.active_grid
            {
                *command = Some(EditorCommand::OpenDataGrid(grid));
            }
        });
        ui.separator();
        if !matches!(editor.state.flat_view, FlatViewTarget::None) {
            flat_panel(ui, editor, flat_cache);
        } else if let Some(grid) = editor.state.active_grid {
            data_panel(ui, editor, grid, clipboard, command);
        } else {
            ui.label("Select a section/chart for a flat view, or a composition grid for data.");
        }
    }

    fn effective_target(editor: &TetraplotEditor) -> Option<FlatViewTarget> {
        match editor.state.flat_view {
            FlatViewTarget::Section(id) => Some(FlatViewTarget::Section(id)),
            FlatViewTarget::EmbeddedChart(id) => Some(FlatViewTarget::EmbeddedChart(id)),
            FlatViewTarget::FollowSelection => match editor.state.selection {
                Selection::Section(id) => Some(FlatViewTarget::Section(id)),
                Selection::SectionSeries { section, .. } => Some(FlatViewTarget::Section(section)),
                Selection::EmbeddedChart(id)
                | Selection::EmbeddedSeries { chart: id, .. }
                | Selection::SurfacePatch { chart: id, .. }
                | Selection::BreakLine { chart: id, .. } => Some(FlatViewTarget::EmbeddedChart(id)),
                _ => None,
            },
            FlatViewTarget::None => None,
        }
    }
    fn flat_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        cache: &mut Option<FlatTexture>,
    ) {
        let Some(target) = effective_target(editor) else {
            ui.label("Select a planar section or embedded chart to open its local ternary view.");
            return;
        };
        let size = ui.available_size();
        let width = size.x.clamp(200.0, 900.0) as u32;
        let height = size.y.clamp(180.0, 500.0) as u32;
        let key = (
            target,
            editor.document.revision(),
            editor.state.revision(),
            width,
            height,
        );
        let stale = cache.as_ref().is_none_or(|cache| cache.key != key);
        if stale {
            match editor
                .document
                .plot()
                .flat_chart(target)
                .and_then(|chart| chart.render((width, height)))
            {
                Ok(image) => {
                    let color = egui::ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        image.rgba(),
                    );
                    let texture =
                        ui.ctx()
                            .load_texture("flat-ternary", color, egui::TextureOptions::LINEAR);
                    *cache = Some(FlatTexture {
                        key,
                        image,
                        texture,
                    });
                }
                Err(error) => {
                    editor.state.status = Some(error.to_string());
                    return;
                }
            }
        }
        let Some(cache) = cache.as_ref() else {
            return;
        };
        let response = ui
            .image((cache.texture.id(), egui::vec2(width as f32, height as f32)))
            .interact(egui::Sense::hover());
        if response.hovered() {
            if let Some(pointer) = response.hover_pos() {
                let scale_x = width as f32 / response.rect.width();
                let scale_y = height as f32 / response.rect.height();
                let local = cache.image.local_at_pixel([
                    (pointer.x - response.rect.min.x) * scale_x,
                    (pointer.y - response.rect.min.y) * scale_y,
                ]);
                if let Some(local) = local {
                    set_linked_cursor(editor, target, local);
                }
            }
        } else {
            editor.state.clear_cursor();
        }
    }
    fn set_linked_cursor(
        editor: &mut TetraplotEditor,
        target: FlatViewTarget,
        local: crate::TernaryPoint,
    ) {
        let tolerance = editor.document.plot().tolerance();
        let geometry = editor.document.plot().geometry();
        let cursor = match target {
            FlatViewTarget::Section(id) => editor
                .document
                .plot()
                .section(id)
                .and_then(|section| section.embedding().ok())
                .and_then(|embedding| {
                    embedding
                        .map(local, tolerance)
                        .ok()
                        .map(|tetrahedral| LinkedCursor {
                            owner: LinkedCursorOwner::Section(id),
                            local_position: local,
                            tetrahedral_position: tetrahedral,
                            world_position: geometry.to_world(tetrahedral),
                            surface_triangle: None,
                            patch: None,
                        })
                }),
            FlatViewTarget::EmbeddedChart(id) => {
                editor.document.plot().embedded_chart(id).and_then(|chart| {
                    chart
                        .embedding()
                        .locate(local, tolerance)
                        .ok()
                        .map(|located| LinkedCursor {
                            owner: LinkedCursorOwner::EmbeddedChart(id),
                            local_position: local,
                            tetrahedral_position: located.tetrahedral,
                            world_position: geometry.to_world(located.tetrahedral),
                            surface_triangle: Some(located.triangle),
                            patch: Some(located.patch),
                        })
                })
            }
            _ => None,
        };
        editor.state.linked_cursor = cursor;
    }

    fn data_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        grid_id: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
        command: &mut Option<EditorCommand>,
    ) {
        let Some(grid) = editor.document.grid(grid_id) else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(grid.name());
            if ui.button("Copy compositions").clicked() {
                match grid.table(&GridExportOptions {
                    scalar_fields: Vec::new(),
                    ..GridExportOptions::default()
                }) {
                    Ok(table) => match clipboard.as_mut() {
                        Some(clipboard) => {
                            if let Err(error) = clipboard.set_text(table.to_tsv(true)) {
                                editor.state.status = Some(error.to_string())
                            }
                        }
                        None => {
                            editor.state.status =
                                Some("System clipboard is unavailable on this platform.".to_owned())
                        }
                    },
                    Err(error) => editor.state.status = Some(error.to_string()),
                }
            }
            if ui.button("Copy full TSV").clicked()
                && let Some(clipboard) = clipboard.as_mut()
                && let Ok(table) = grid.table(&GridExportOptions::default())
            {
                let _ = clipboard.set_text(table.to_tsv(true));
            }
        });
        let fields: Vec<_> = grid
            .fields()
            .iter()
            .map(|field| (field.id(), field.name().to_owned()))
            .collect();
        let rows: Vec<_> = grid.row_ids();
        egui::ScrollArea::vertical().show_rows(ui, 20.0, rows.len(), |ui, range| {
            for index in range {
                let row = rows[index];
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            editor.state.selection == Selection::GridRow { grid: grid_id, row },
                            row.get().to_string(),
                        )
                        .clicked()
                    {
                        *command = Some(EditorCommand::SetSelection(Selection::GridRow {
                            grid: grid_id,
                            row,
                        }));
                    }
                    if let Ok(Some(coordinate)) = grid.row_coordinate(row) {
                        for value in &coordinate.as_values()[..coordinate.dimension()] {
                            ui.label(format!("{value:.4}"));
                        }
                    }
                    for (field, name) in &fields {
                        let mut text = grid
                            .scalar_values(row)
                            .ok()
                            .and_then(|values| {
                                grid.fields()
                                    .iter()
                                    .position(|candidate| candidate.id() == *field)
                                    .and_then(|position| values[position])
                            })
                            .map(|value| value.to_string())
                            .unwrap_or_default();
                        if ui
                            .add(egui::TextEdit::singleline(&mut text).hint_text(name))
                            .lost_focus()
                        {
                            let value = if text.trim().is_empty() {
                                None
                            } else {
                                text.parse().ok()
                            };
                            *command = Some(EditorCommand::EditGridScalar {
                                grid: grid_id,
                                row,
                                field: *field,
                                value,
                            });
                        }
                    }
                });
            }
        });
    }

    fn handle_3d_pick(
        events: &[three_d::Event],
        camera: &NativeCamera,
        editor: &mut TetraplotEditor,
    ) {
        let click = events.iter().find_map(|event| match event {
            three_d::Event::MousePress {
                button: three_d::MouseButton::Left,
                position,
                handled: false,
                ..
            } => Some(*position),
            _ => None,
        });
        let Some(pixel) = click else {
            return;
        };
        let origin = camera.position_at_pixel(pixel);
        let direction = camera.view_direction_at_pixel(pixel);
        let Some(ray) = crate::Ray::new(
            [
                f64::from(origin.x),
                f64::from(origin.y),
                f64::from(origin.z),
            ],
            [
                f64::from(direction.x),
                f64::from(direction.y),
                f64::from(direction.z),
            ],
        ) else {
            return;
        };
        let geometry = editor.document.plot().geometry();
        let tolerance = editor.document.plot().tolerance();
        let mut candidates = Vec::new();
        for chart in editor.document.plot().embedded_charts() {
            let Some(id) = chart.id() else {
                continue;
            };
            let Ok(prepared) = chart.prepared(&geometry, tolerance) else {
                continue;
            };
            if let Some(hit) = crate::pick_embedded_chart(
                ray,
                id,
                chart.embedding(),
                &prepared,
                &geometry,
                tolerance,
            ) {
                candidates.push(hit);
            }
        }
        for section in editor.document.plot().sections() {
            let Some(id) = section.id() else {
                continue;
            };
            let Ok(prepared) = section.prepared(&geometry, tolerance) else {
                continue;
            };
            if let Some(hit) = crate::pick_prepared_surface(ray, &prepared, &geometry, tolerance) {
                candidates.push(crate::EditorPickResult {
                    selection: Selection::Section(id),
                    local_position: Some(hit.local),
                    tetrahedral_position: Some(hit.tetrahedral),
                    world_position: hit.world,
                    surface_triangle: Some(hit.surface_triangle),
                    patch: Some(hit.patch),
                    series_id: None,
                    primitive_index: Some(hit.surface_triangle as usize),
                    grid: None,
                    grid_row: None,
                    scalar_value: None,
                    break_line: hit.break_line,
                });
            }
        }
        let chosen = candidates.into_iter().min_by(|left, right| {
            let left_distance = squared_distance(ray.origin, left.world_position);
            let right_distance = squared_distance(ray.origin, right.world_position);
            left_distance.total_cmp(&right_distance)
        });
        if let Some(hit) = chosen {
            editor.state.select(hit.selection.clone());
            if let (Some(local), Some(tetrahedral)) = (hit.local_position, hit.tetrahedral_position)
            {
                let owner = match hit.selection {
                    Selection::Section(id) => Some(LinkedCursorOwner::Section(id)),
                    Selection::SurfacePatch { chart, .. } | Selection::EmbeddedChart(chart) => {
                        Some(LinkedCursorOwner::EmbeddedChart(chart))
                    }
                    _ => None,
                };
                if let Some(owner) = owner {
                    editor.state.linked_cursor = Some(LinkedCursor {
                        owner,
                        local_position: local,
                        tetrahedral_position: tetrahedral,
                        world_position: hit.world_position,
                        surface_triangle: hit.surface_triangle,
                        patch: hit.patch,
                    });
                }
            }
        }
    }
    fn squared_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
        left.into_iter()
            .zip(right)
            .map(|(left, right)| (left - right).powi(2))
            .sum()
    }
    fn backend(error: impl std::fmt::Display) -> crate::TetraplotError {
        crate::RenderError::Backend {
            message: error.to_string(),
        }
        .into()
    }
    fn vec3(value: [f32; 3]) -> three_d::Vec3 {
        three_d::vec3(value[0], value[1], value[2])
    }
}
