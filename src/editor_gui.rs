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
        let mut models = crate::render::three_d_backend::editor_models(
            &editor.document,
            &editor.state,
            &context,
        )?;
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
        let mut model_key = (
            editor.document.revision(),
            editor.state.selection_revision(),
            editor.state.cursor_revision(),
        );
        let mut flat_cache: Option<FlatTexture> = None;
        let started = Instant::now();
        window.render_loop(move |mut frame| {
            let mut interaction = UiInteraction::default();
            let window_size = [frame.viewport.width, frame.viewport.height];
            gui.update(
                &mut frame.events,
                started.elapsed().as_secs_f64() * 1000.0,
                frame.viewport,
                frame.device_pixel_ratio,
                |ui| {
                    interaction = ui_shell(
                        ui,
                        &mut editor,
                        &mut clipboard,
                        &mut flat_cache,
                        window_size,
                        frame.device_pixel_ratio,
                    );
                },
            );
            if let Some(command) = interaction.command.take() {
                match command.execute(&mut editor.document, &mut editor.state) {
                    Ok(Some(summary)) => {
                        editor.state.status = Some(format!(
                            "Paste: {} updated, {} inserted, {} valid, {} invalid, {} incomplete{}",
                            summary.updated,
                            summary.inserted,
                            summary.valid,
                            summary.invalid,
                            summary.incomplete,
                            if summary.warnings.is_empty() {
                                String::new()
                            } else {
                                format!("; {}", summary.warnings.join("; "))
                            }
                        ));
                    }
                    Ok(None) => {}
                    Err(error) => editor.state.status = Some(error.to_string()),
                }
            }
            editor.state.clear_removed(&editor.document);
            if let Some(viewport) = interaction.viewport {
                let physical = viewport.physical_rect;
                camera.set_viewport(three_d::Viewport {
                    x: physical.x,
                    y: physical.y,
                    width: physical.width,
                    height: physical.height,
                });
                restrict_events_to_viewport(&mut frame.events, viewport);
                handle_3d_pick(&frame.events, &camera, viewport, &mut editor);
                orbit.handle_events(&mut camera, &mut frame.events);
            }
            let next_model_key = (
                editor.document.revision(),
                editor.state.selection_revision(),
                editor.state.cursor_revision(),
            );
            if next_model_key != model_key {
                match crate::render::three_d_backend::editor_models(
                    &editor.document,
                    &editor.state,
                    &context,
                ) {
                    Ok(updated) => {
                        models = updated;
                        model_key = next_model_key;
                    }
                    Err(error) => editor.state.status = Some(error.to_string()),
                }
            }
            let background = editor.document.plot().background().clamped();
            let screen = frame.screen();
            screen.clear(ClearState::color_and_depth(
                background.red(),
                background.green(),
                background.blue(),
                1.0,
                1.0,
            ));
            if let Some(viewport) = interaction.viewport {
                let physical = viewport.physical_rect;
                screen.render_partially(
                    three_d::ScissorBox {
                        x: physical.x,
                        y: physical.y,
                        width: physical.width,
                        height: physical.height,
                    },
                    &camera,
                    &models,
                    &[],
                );
            }
            if let Err(error) = screen.write(|| gui.render()) {
                editor.state.status = Some(error.to_string());
            }
            FrameOutput::default()
        });
        Ok(())
    }

    #[derive(Default)]
    struct UiInteraction {
        command: Option<EditorCommand>,
        viewport: Option<crate::EditorViewport>,
    }

    struct FlatTexture {
        key: (FlatViewTarget, u64, u64, u32, u32),
        image: FlatChartImage,
        points: Vec<crate::FlatGridPoint>,
        texture: egui::TextureHandle,
    }

    fn ui_shell(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        flat_cache: &mut Option<FlatTexture>,
        window_size: [u32; 2],
        device_pixel_ratio: f32,
    ) -> UiInteraction {
        let mut command = None;
        egui::Panel::top("editor-menu").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("tetraplot editor").strong());
                if ui.button("Fit scene").clicked() {
                    command = Some(EditorCommand::FitCamera)
                }
                if ui.button("Reset camera").clicked() {
                    command = Some(EditorCommand::ResetCamera)
                }
                #[cfg(feature = "image-export")]
                if ui.button("Save 3D PNG").clicked() {
                    editor.state.status = editor
                        .document
                        .plot()
                        .save_png("tetraplot-editor-scene.png", (1600, 1200))
                        .err()
                        .map(|error| error.to_string())
                        .or(Some("Saved tetraplot-editor-scene.png".to_owned()));
                }
                #[cfg(not(feature = "image-export"))]
                ui.add_enabled(false, egui::Button::new("Save 3D PNG"))
                    .on_hover_text("Enable the image-export feature for PNG output.");
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
            .default_size(275.0)
            .resizable(true)
            .show_inside(ui, |ui| properties(ui, editor, clipboard, &mut command));
        egui::Panel::bottom("status")
            .exact_size(24.0)
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
        egui::Panel::bottom("lower-panel")
            .default_size(320.0)
            .resizable(true)
            .show_inside(ui, |ui| {
                lower_panel(ui, editor, clipboard, flat_cache, &mut command)
            });
        let central = egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let rect = ui.available_rect_before_wrap();
                ui.allocate_rect(rect, egui::Sense::hover());
                ui.painter().text(
                    rect.left_top() + egui::vec2(10.0, 8.0),
                    egui::Align2::LEFT_TOP,
                    "3D viewport · drag to orbit · wheel to zoom · click to pick",
                    egui::FontId::proportional(12.0),
                    egui::Color32::from_white_alpha(190),
                );
                rect
            })
            .inner;
        let viewport = crate::EditorViewport::from_logical(
            crate::LogicalViewportRect {
                min: [central.min.x, central.min.y],
                max: [central.max.x, central.max.y],
            },
            window_size,
            device_pixel_ratio,
        );
        UiInteraction { command, viewport }
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
        clipboard: &mut Option<arboard::Clipboard>,
        command: &mut Option<EditorCommand>,
    ) {
        ui.heading("Properties");
        match editor.state.selection.clone() {
            Selection::None => {
                ui.label("Select a scene object.");
            }
            Selection::Frame => {
                let style = editor.document.plot().frame();
                if let Some(color) = edit_color(ui, "Edge colour", style.edge_color()) {
                    *command = Some(EditorCommand::SetFrameEdgeColor(color));
                }
                let mut width = style.edge_width();
                if ui
                    .add(egui::Slider::new(&mut width, 0.1..=8.0).text("Edge width"))
                    .changed()
                {
                    *command = Some(EditorCommand::SetFrameEdgeWidth(width));
                }
                let mut faces = style.faces();
                if ui.checkbox(&mut faces, "Show faces").changed() {
                    *command = Some(EditorCommand::SetFrameFacesVisible(faces));
                }
                if let Some(color) = edit_color(ui, "Face colour", style.face_color()) {
                    *command = Some(EditorCommand::SetFrameFaceColor(color));
                }
                if let Some(color) =
                    edit_color(ui, "Background", editor.document.plot().background())
                {
                    *command = Some(EditorCommand::SetBackground(color));
                }
            }
            Selection::Section(id) => {
                let Some(section) = editor.document.plot().section(id) else {
                    return;
                };
                let mut name = section.label().unwrap_or("Planar section").to_owned();
                let plane = section.plane();
                let style = section.style();
                let mut visible = section.visible();
                ui.label(format!("Section {}", id.get()));
                if ui.text_edit_singleline(&mut name).changed() {
                    *command = Some(EditorCommand::SetSectionName {
                        section: id,
                        name: Some(name),
                    });
                }
                if ui.checkbox(&mut visible, "Visible").changed() {
                    *command = Some(EditorCommand::SetVisibility {
                        target: Selection::Section(id),
                        visible,
                    });
                }
                if let crate::SectionPlane::ConstantComponent {
                    component,
                    mut value,
                } = plane
                    && ui
                        .add(
                            egui::DragValue::new(&mut value)
                                .range(0.0..=1.0)
                                .speed(0.005)
                                .prefix(format!("{component:?} = ")),
                        )
                        .changed()
                {
                    *command = Some(EditorCommand::SetSectionValue { section: id, value });
                }
                let mut opacity = style.fill_opacity;
                if ui
                    .add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Fill opacity"))
                    .changed()
                {
                    *command = Some(EditorCommand::SetOpacity {
                        target: Selection::Section(id),
                        opacity,
                    });
                }
                let mut boundary = style.boundary_visible;
                if ui.checkbox(&mut boundary, "Boundary").changed() {
                    *command = Some(EditorCommand::SetEmbeddedBoundaryVisible {
                        target: Selection::Section(id),
                        visible: boundary,
                    });
                }
                let mut grid = style.grid.visible;
                if ui.checkbox(&mut grid, "Ternary grid").changed() {
                    *command = Some(EditorCommand::SetEmbeddedGridVisible {
                        target: Selection::Section(id),
                        visible: grid,
                    });
                }
                if ui.button("Open flat view").clicked() {
                    *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::Section(id)));
                }
                if ui.button("Remove section").clicked() {
                    *command = Some(EditorCommand::RemoveSection(id));
                }
            }
            Selection::EmbeddedChart(id) => {
                let Some(chart) = editor.document.plot().embedded_chart(id) else {
                    return;
                };
                let mut name = chart
                    .label()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Embedded chart {}", id.get()));
                let mut visible = chart.visible();
                let style = chart.style();
                if ui.text_edit_singleline(&mut name).changed() {
                    *command = Some(EditorCommand::SetEmbeddedChartName {
                        chart: id,
                        name: Some(name),
                    });
                }
                if ui.checkbox(&mut visible, "Visible").changed() {
                    *command = Some(EditorCommand::SetVisibility {
                        target: Selection::EmbeddedChart(id),
                        visible,
                    });
                }
                let mut surface_visible = style.surface_visible;
                if ui
                    .checkbox(&mut surface_visible, "Supporting surface")
                    .changed()
                {
                    *command = Some(EditorCommand::SetEmbeddedSurfaceVisible {
                        target: Selection::EmbeddedChart(id),
                        visible: surface_visible,
                    });
                }
                let mut opacity = style.fill_opacity;
                if ui
                    .add(egui::Slider::new(&mut opacity, 0.0..=1.0).text("Fill opacity"))
                    .changed()
                {
                    *command = Some(EditorCommand::SetOpacity {
                        target: Selection::EmbeddedChart(id),
                        opacity,
                    });
                }
                let mut grid = style.grid.visible;
                if ui.checkbox(&mut grid, "Ternary grid").changed() {
                    *command = Some(EditorCommand::SetEmbeddedGridVisible {
                        target: Selection::EmbeddedChart(id),
                        visible: grid,
                    });
                }
                let mut boundary = style.boundary_visible;
                if ui.checkbox(&mut boundary, "Boundary").changed() {
                    *command = Some(EditorCommand::SetEmbeddedBoundaryVisible {
                        target: Selection::EmbeddedChart(id),
                        visible: boundary,
                    });
                }
                let mut normal_mode = style.normal_mode;
                egui::ComboBox::from_id_salt(("normal-mode", id.get()))
                    .selected_text(format!("{normal_mode:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut normal_mode,
                            crate::SurfaceNormalMode::Flat,
                            "Flat",
                        );
                        ui.selectable_value(
                            &mut normal_mode,
                            crate::SurfaceNormalMode::SmoothWithinPatches,
                            "Smooth within patches",
                        );
                    });
                if normal_mode != style.normal_mode {
                    *command = Some(EditorCommand::SetEmbeddedNormalMode {
                        target: Selection::EmbeddedChart(id),
                        mode: normal_mode,
                    });
                }
                if ui.button("Open flat view").clicked() {
                    *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::EmbeddedChart(
                        id,
                    )));
                }
                if ui.button("Remove chart").clicked() {
                    *command = Some(EditorCommand::RemoveEmbeddedChart(id));
                }
            }
            Selection::SurfacePatch { chart, patch } => {
                ui.label(format!("Patch {}", patch.get()));
                ui.label(format!("Chart {}", chart.get()));
            }
            Selection::BreakLine { chart, line } => {
                let Some(embedding) = editor
                    .document
                    .plot()
                    .embedded_chart(chart)
                    .and_then(|chart| chart.embedding().as_triangulated())
                else {
                    return;
                };
                let Some(source) = embedding
                    .break_lines()
                    .iter()
                    .find(|candidate| candidate.id() == line)
                else {
                    return;
                };
                let mut kind = source.kind();
                let mut style = source.style();
                let adjacent = source.adjacent_patches();
                ui.label(format!("Break line {}", line.get()));
                ui.label(format!(
                    "Adjacent patches: {} and {}",
                    adjacent[0].get(),
                    adjacent[1].get()
                ));
                egui::ComboBox::from_id_salt(("break-kind", chart.get(), line.get()))
                    .selected_text(format!("{kind:?}"))
                    .show_ui(ui, |ui| {
                        for candidate in [
                            crate::BreakLineKind::Crease,
                            crate::BreakLineKind::Univariant,
                            crate::BreakLineKind::PhaseBoundary,
                            crate::BreakLineKind::UserDefined,
                        ] {
                            ui.selectable_value(&mut kind, candidate, format!("{candidate:?}"));
                        }
                    });
                if kind != source.kind() {
                    *command = Some(EditorCommand::SetBreakLineKind { chart, line, kind });
                }
                let mut visible = style.visible;
                if ui.checkbox(&mut visible, "Visible").changed() {
                    style.visible = visible;
                    *command = Some(EditorCommand::SetBreakLineStyle { chart, line, style });
                }
                let mut width = style.width;
                if ui
                    .add(egui::Slider::new(&mut width, 0.5..=10.0).text("Width"))
                    .changed()
                {
                    style.width = width;
                    *command = Some(EditorCommand::SetBreakLineStyle { chart, line, style });
                }
                if let Some(color) = edit_color(ui, "Colour", style.color) {
                    style.color = color;
                    *command = Some(EditorCommand::SetBreakLineStyle { chart, line, style });
                }
            }
            Selection::Grid(id) => {
                let Some(grid) = editor.document.grid(id) else {
                    return;
                };
                let mut name = grid.name().to_owned();
                let space = grid.coordinate_space();
                let summary = grid.validation_summary();
                let fields: Vec<_> = grid
                    .fields()
                    .iter()
                    .map(|field| (field.id(), field.name().to_owned()))
                    .collect();
                if ui.text_edit_singleline(&mut name).changed() {
                    *command = Some(EditorCommand::SetGridName { grid: id, name });
                }
                ui.label(format!("Coordinate space: {space:?}"));
                if let Some(mut definition) = grid.regular_definition() {
                    ui.label("Ordering: Lexicographic");
                    let mut policy = editor.state.table_state_mut(id).redefinition_policy;
                    egui::ComboBox::from_id_salt(("redefinition-policy", id.get()))
                        .selected_text(match policy {
                            crate::GridRedefinitionPolicy::ClearScalars => {
                                "On resize: clear scalars"
                            }
                            crate::GridRedefinitionPolicy::PreserveMatchingCoordinates => {
                                "On resize: preserve matching"
                            }
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut policy,
                                crate::GridRedefinitionPolicy::PreserveMatchingCoordinates,
                                "Preserve matching coordinates",
                            );
                            ui.selectable_value(
                                &mut policy,
                                crate::GridRedefinitionPolicy::ClearScalars,
                                "Clear scalar values",
                            );
                        });
                    editor.state.table_state_mut(id).redefinition_policy = policy;
                    let subdivisions = match &mut definition {
                        crate::RegularGridDefinition::Tetrahedral(value) => &mut value.subdivisions,
                        crate::RegularGridDefinition::LocalTernary(value) => {
                            &mut value.subdivisions
                        }
                    };
                    if ui
                        .add(
                            egui::DragValue::new(subdivisions)
                                .range(1..=512)
                                .prefix("Subdivisions: "),
                        )
                        .changed()
                    {
                        *command = Some(EditorCommand::RedefineRegularGrid {
                            grid: id,
                            definition,
                            policy,
                        });
                    }
                }
                ui.label(format!(
                    "{} valid, {} incomplete, {} invalid",
                    summary.valid, summary.incomplete, summary.invalid
                ));
                for (field, name) in fields {
                    if ui.button(format!("Scalar: {name}")).clicked() {
                        *command = Some(EditorCommand::SetSelection(Selection::ScalarField {
                            grid: id,
                            field,
                        }));
                    }
                }
                if ui.button("Open data table").clicked() {
                    *command = Some(EditorCommand::OpenDataGrid(id));
                }
                if ui.button("Export TSV").clicked() {
                    *command = Some(EditorCommand::ExportGridTsv {
                        grid: id,
                        path: format!("tetraplot-grid-{}.tsv", id.get()).into(),
                        options: GridExportOptions::default(),
                    });
                }
                if ui.button("Remove grid").clicked() {
                    *command = Some(EditorCommand::RemoveGrid(id));
                }
            }
            Selection::ScalarField { grid, field } => {
                let Some(source) = editor
                    .document
                    .grid(grid)
                    .and_then(|grid| grid.field(field))
                else {
                    return;
                };
                let mut name = source.name().to_owned();
                let mut units = source.units().unwrap_or_default().to_owned();
                let mut precision = source.precision();
                ui.label(format!("Scalar field {}", field.get()));
                if ui.text_edit_singleline(&mut name).changed() {
                    *command = Some(EditorCommand::RenameScalarField { grid, field, name });
                }
                ui.horizontal(|ui| {
                    ui.label("Units");
                    if ui.text_edit_singleline(&mut units).changed() {
                        *command = Some(EditorCommand::SetScalarUnits {
                            grid,
                            field,
                            units: (!units.trim().is_empty()).then_some(units.clone()),
                        });
                    }
                });
                if ui
                    .add(egui::Slider::new(&mut precision, 0..=15).text("Precision"))
                    .changed()
                {
                    *command = Some(EditorCommand::SetScalarPrecision {
                        grid,
                        field,
                        precision,
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("Copy field").clicked() {
                        let output = editor.document.grid(grid).and_then(|value| {
                            value
                                .table(&GridExportOptions {
                                    include_compositions: false,
                                    scalar_fields: vec![field],
                                    ..GridExportOptions::default()
                                })
                                .ok()
                        });
                        write_clipboard(editor, clipboard, output.map(|table| table.to_tsv(true)));
                    }
                    if ui.button("Paste field").clicked() {
                        let text = clipboard
                            .as_mut()
                            .and_then(|clipboard| clipboard.get_text().ok());
                        let start = editor
                            .state
                            .table_state(grid)
                            .and_then(|table| table.active_cell)
                            .map(|cell| cell.row)
                            .or_else(|| {
                                editor
                                    .document
                                    .grid(grid)
                                    .and_then(|value| value.row_ids().first().copied())
                            });
                        match (text, start) {
                            (Some(text), Some(start)) => {
                                *command = Some(EditorCommand::PasteScalarColumn {
                                    grid,
                                    field,
                                    start,
                                    clipboard: parse_clipboard_for_grid(
                                        &text,
                                        editor.document.grid(grid),
                                    ),
                                });
                            }
                            _ => {
                                editor.state.status =
                                    Some("No scalar paste destination is available.".to_owned());
                            }
                        }
                    }
                });
                if ui.button("Clear values").clicked() {
                    *command = Some(EditorCommand::ClearScalarField { grid, field });
                }
                if ui.button("Remove field").clicked() {
                    *command = Some(EditorCommand::RemoveScalarField { grid, field });
                }
            }
            Selection::GridRow { grid, row } => {
                ui.label(format!("Grid {} row {}", grid.get(), row.get()));
                if let Some(source) = editor.document.grid(grid) {
                    if let Ok(Some(coordinate)) = source.row_coordinate(row) {
                        ui.label(format!(
                            "Scientific coordinate: {:?}",
                            coordinate.as_values()
                        ));
                    }
                    if let Ok(values) = source.scalar_values(row) {
                        ui.label(format!("Scalar values: {values:?}"));
                    }
                }
            }
            Selection::GridCell { grid, row, column } => {
                ui.label(format!(
                    "Grid {} · row {} · column {}",
                    grid.get(),
                    row.get(),
                    column.get()
                ));
            }
            _ => {
                ui.label("Properties are available for this selection.");
            }
        }
    }

    fn edit_color(ui: &mut egui::Ui, label: &str, color: crate::Color) -> Option<crate::Color> {
        let mut rgba = [color.red(), color.green(), color.blue(), color.alpha()];
        let changed = ui
            .horizontal(|ui| {
                ui.label(label);
                ui.color_edit_button_rgba_unmultiplied(&mut rgba).changed()
            })
            .inner;
        changed.then_some(crate::Color::new(rgba[0], rgba[1], rgba[2], rgba[3]))
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
                editor.state.set_flat_view(FlatViewTarget::None);
                *command = Some(EditorCommand::OpenDataGrid(grid));
            }
        });
        ui.separator();
        if !matches!(editor.state.flat_view, FlatViewTarget::None) {
            flat_panel(ui, editor, flat_cache, command);
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
                Selection::Grid(id)
                | Selection::GridRow { grid: id, .. }
                | Selection::GridCell { grid: id, .. }
                | Selection::ScalarField { grid: id, .. } => {
                    editor
                        .document
                        .grid(id)
                        .and_then(|grid| match grid.coordinate_space() {
                            crate::GridCoordinateSpace::Section(id) => {
                                Some(FlatViewTarget::Section(id))
                            }
                            crate::GridCoordinateSpace::EmbeddedChart(id) => {
                                Some(FlatViewTarget::EmbeddedChart(id))
                            }
                            crate::GridCoordinateSpace::Tetrahedral => None,
                        })
                }
                _ => None,
            },
            FlatViewTarget::None => None,
        }
    }
    fn flat_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        cache: &mut Option<FlatTexture>,
        command: &mut Option<EditorCommand>,
    ) {
        let Some(target) = effective_target(editor) else {
            ui.label("Select a planar section, embedded chart, or attached grid.");
            return;
        };
        let size = ui.available_size();
        let width = size.x.clamp(200.0, 900.0) as u32;
        let height = size.y.clamp(180.0, 500.0) as u32;
        let highlight_revision = editor
            .state
            .selection_revision()
            .wrapping_mul(31)
            .wrapping_add(editor.state.cursor_revision());
        let key = (
            target,
            editor.document.flat_revision(target),
            highlight_revision,
            width,
            height,
        );
        let stale = cache.as_ref().is_none_or(|cache| cache.key != key);
        if stale {
            let points = flat_grid_points(editor, target);
            let chart = editor.document.plot().flat_chart(target);
            match chart.and_then(|mut chart| {
                let options = chart.options_mut();
                options.grid_points = points.clone();
                options.selected_series = match (target, &editor.state.selection) {
                    (
                        FlatViewTarget::Section(owner),
                        Selection::SectionSeries { section, series },
                    ) if owner == *section => Some(*series),
                    (
                        FlatViewTarget::EmbeddedChart(owner),
                        Selection::EmbeddedSeries { chart, series },
                    ) if owner == *chart => Some(*series),
                    _ => None,
                };
                options.selected_break = match (target, &editor.state.selection) {
                    (
                        FlatViewTarget::EmbeddedChart(owner),
                        Selection::BreakLine { chart, line },
                    ) if owner == *chart => Some(*line),
                    _ => None,
                };
                options.linked_cursor = editor.state.linked_cursor.as_ref().and_then(|cursor| {
                    let applies = matches!(
                        (target, &cursor.owner),
                        (FlatViewTarget::Section(left), LinkedCursorOwner::Section(right)) if left == *right
                    ) || matches!(
                        (target, &cursor.owner),
                        (
                            FlatViewTarget::EmbeddedChart(left),
                            LinkedCursorOwner::EmbeddedChart(right)
                        ) if left == *right
                    );
                    applies.then_some(cursor.local_position)
                });
                chart.render((width, height))
            }) {
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
                        points,
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
            .interact(egui::Sense::click());
        if response.hovered() {
            if let Some(pointer) = response.hover_pos() {
                let scale_x = width as f32 / response.rect.width();
                let scale_y = height as f32 / response.rect.height();
                let image_pixel = [
                    (pointer.x - response.rect.min.x) * scale_x,
                    (pointer.y - response.rect.min.y) * scale_y,
                ];
                if response.clicked()
                    && let Some(point) = cache
                        .points
                        .iter()
                        .filter_map(|point| {
                            let pixel = cache.image.pixel_at_local(point.local)?;
                            let distance = (pixel[0] - image_pixel[0]).powi(2)
                                + (pixel[1] - image_pixel[1]).powi(2);
                            Some((distance, point))
                        })
                        .filter(|(distance, _)| *distance <= 12.0f32.powi(2))
                        .min_by(|left, right| left.0.total_cmp(&right.0))
                        .map(|(_, point)| point)
                {
                    *command = Some(EditorCommand::SetSelection(Selection::GridRow {
                        grid: point.grid,
                        row: point.row,
                    }));
                }
                if let Some(local) = cache.image.local_at_pixel(image_pixel) {
                    set_linked_cursor(editor, target, local);
                }
            }
        } else {
            editor.state.clear_cursor();
        }
    }

    fn flat_grid_points(
        editor: &TetraplotEditor,
        target: FlatViewTarget,
    ) -> Vec<crate::FlatGridPoint> {
        let selected = match editor.state.selection {
            Selection::GridRow { grid, row } | Selection::GridCell { grid, row, .. } => {
                Some((grid, row))
            }
            _ => None,
        };
        editor
            .document
            .grids()
            .filter(|grid| {
                matches!(
                    (target, grid.coordinate_space()),
                    (
                        FlatViewTarget::Section(left),
                        crate::GridCoordinateSpace::Section(right)
                    ) if left == right
                ) || matches!(
                    (target, grid.coordinate_space()),
                    (
                        FlatViewTarget::EmbeddedChart(left),
                        crate::GridCoordinateSpace::EmbeddedChart(right)
                    ) if left == right
                )
            })
            .filter_map(|grid| grid.id())
            .flat_map(|grid| {
                editor
                    .document
                    .prepared_grid_points(grid)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(move |point| {
                        point.local.map(|local| crate::FlatGridPoint {
                            grid,
                            row: point.row_id,
                            local,
                            selected: selected == Some((grid, point.row_id)),
                        })
                    })
            })
            .collect()
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
        editor.state.set_cursor(cursor);
    }

    fn data_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        grid_id: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
        command: &mut Option<EditorCommand>,
    ) {
        editor.state.table_state_mut(grid_id);
        let Some(grid) = editor.document.grid(grid_id) else {
            editor.state.status = Some("The selected grid no longer exists.".to_owned());
            return;
        };
        let columns = crate::grid_columns(grid);
        let table_snapshot = editor
            .state
            .table_state(grid_id)
            .cloned()
            .unwrap_or_else(|| crate::DataTableState::new(grid_id));
        let rows = table_snapshot.displayed_rows(grid);
        let selected = table_snapshot.selected_addresses(&rows, &columns);
        let validation = grid.validation_summary();
        let grid_name = grid.name().to_owned();
        let is_regular = grid.is_regular();
        let entry_mode = grid.entry_mode();
        let duplicate_policy = grid.duplicate_policy();
        let row_values: Vec<_> = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .map(|column| crate::cell_text(grid, *row, column).unwrap_or_default())
                    .collect::<Vec<_>>()
            })
            .collect();
        let table_focus = egui::Id::new(("grid-table-focus", grid_id.get()));

        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(&grid_name).strong());
            if let Err(error) = editor.document.prepared_grid_points(grid_id) {
                ui.label(
                    egui::RichText::new(format!("Mapping: {error}"))
                        .color(egui::Color32::from_rgb(235, 116, 91)),
                );
            }
            ui.label(format!(
                "{} valid · {} incomplete · {} invalid · {} warnings",
                validation.valid, validation.incomplete, validation.invalid, validation.warnings
            ));
            if ui.button("Copy").clicked() {
                copy_table_selection(editor, grid_id, clipboard, false, "Copied selected cells");
            }
            if ui.button("Copy with headers").clicked() {
                copy_table_selection(
                    editor,
                    grid_id,
                    clipboard,
                    true,
                    "Copied selected cells with headers",
                );
            }
            if ui.button("Copy selected rows").clicked() {
                copy_selected_rows(editor, grid_id, clipboard);
            }
            if ui.button("Copy compositions").clicked() {
                let output = editor.document.grid(grid_id).and_then(|grid| {
                    grid.table(&GridExportOptions {
                        scalar_fields: Vec::new(),
                        ..GridExportOptions::default()
                    })
                    .ok()
                });
                write_clipboard(editor, clipboard, output.map(|table| table.to_tsv(true)));
            }
            if ui.button("Copy full table").clicked() {
                let output = editor
                    .document
                    .grid(grid_id)
                    .and_then(|grid| grid.table(&GridExportOptions::default()).ok());
                write_clipboard(editor, clipboard, output.map(|table| table.to_tsv(true)));
            }
            if ui.button("Paste").clicked() {
                paste_from_clipboard(editor, grid_id, clipboard, false, command);
            }
            if ui.button("Paste transposed").clicked() {
                paste_from_clipboard(editor, grid_id, clipboard, true, command);
            }
            if ui.button("Clear").clicked() {
                *command = Some(EditorCommand::ClearGridCells {
                    grid: grid_id,
                    cells: selected.clone(),
                });
            }
            if ui.button("Fill down").clicked() {
                *command = Some(EditorCommand::FillDownGrid { grid: grid_id });
            }
        });

        if !is_regular {
            ui.horizontal(|ui| {
                if ui.button("Insert row").clicked() {
                    let before = editor
                        .state
                        .table_state(grid_id)
                        .and_then(|table| table.active_cell)
                        .map(|cell| cell.row);
                    *command = Some(EditorCommand::InsertGridRow {
                        grid: grid_id,
                        before,
                    });
                }
                if ui.button("Append row").clicked() {
                    *command = Some(EditorCommand::InsertGridRow {
                        grid: grid_id,
                        before: None,
                    });
                }
                if ui.button("Delete selected rows").clicked() {
                    let mut selected_rows: Vec<_> = selected.iter().map(|cell| cell.row).collect();
                    selected_rows.sort();
                    selected_rows.dedup();
                    *command = Some(EditorCommand::DeleteGridRows {
                        grid: grid_id,
                        rows: selected_rows,
                    });
                }
                if ui.button("Append clipboard").clicked() {
                    match clipboard.as_mut().and_then(|value| value.get_text().ok()) {
                        Some(text) => {
                            let table =
                                parse_clipboard_for_grid(&text, editor.document.grid(grid_id));
                            *command = Some(EditorCommand::AppendGridRows {
                                grid: grid_id,
                                clipboard: table,
                            });
                        }
                        None => {
                            editor.state.status =
                                Some("System clipboard is unavailable or empty.".to_owned())
                        }
                    }
                }
                ui.separator();
                let dimension = if editor
                    .document
                    .grid(grid_id)
                    .is_some_and(|grid| grid.coordinate_space().is_local())
                {
                    3
                } else {
                    4
                };
                let mut selected_mode = entry_mode;
                egui::ComboBox::from_id_salt(("entry-mode", grid_id.get()))
                    .selected_text(entry_mode_label(entry_mode, dimension))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected_mode,
                            crate::CompositionEntryMode::AllComponents,
                            "All components",
                        );
                        for component in [
                            crate::Component::A,
                            crate::Component::B,
                            crate::Component::C,
                            crate::Component::D,
                        ]
                        .into_iter()
                        .take(dimension)
                        {
                            ui.selectable_value(
                                &mut selected_mode,
                                crate::CompositionEntryMode::DependentComponent(component),
                                format!("{} dependent", component_label(component, dimension)),
                            );
                        }
                    });
                if selected_mode != entry_mode {
                    *command = Some(EditorCommand::SetEntryMode {
                        grid: grid_id,
                        mode: selected_mode,
                    });
                }
                let mut selected_policy = duplicate_policy;
                egui::ComboBox::from_id_salt(("duplicate-policy", grid_id.get()))
                    .selected_text(format!("Duplicates: {duplicate_policy:?}"))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected_policy,
                            crate::DuplicateCompositionPolicy::Allow,
                            "Allow duplicates",
                        );
                        ui.selectable_value(
                            &mut selected_policy,
                            crate::DuplicateCompositionPolicy::Warn,
                            "Warn on duplicates",
                        );
                        ui.selectable_value(
                            &mut selected_policy,
                            crate::DuplicateCompositionPolicy::Reject,
                            "Reject duplicates",
                        );
                    });
                if selected_policy != duplicate_policy {
                    *command = Some(EditorCommand::SetDuplicatePolicy {
                        grid: grid_id,
                        policy: selected_policy,
                    });
                }
            });
        }

        ui.horizontal(|ui| {
            if ui.button("Add scalar field").clicked() {
                let count = editor
                    .document
                    .grid(grid_id)
                    .map_or(0, |grid| grid.fields().len());
                *command = Some(EditorCommand::AddScalarField {
                    grid: grid_id,
                    name: format!("Scalar {}", count + 1),
                });
            }
            ui.label("Click a cell; Shift-click extends a rectangular selection.");
        });
        ui.separator();

        ui.horizontal(|ui| {
            for (index, column) in columns.iter().enumerate() {
                let width =
                    table_snapshot
                        .column_widths
                        .get(index)
                        .copied()
                        .unwrap_or(match column.kind {
                            crate::GridColumnKind::Validation => 240.0,
                            crate::GridColumnKind::RowId => 55.0,
                            _ => 105.0,
                        });
                ui.add_sized(
                    [width, 20.0],
                    egui::Label::new(egui::RichText::new(&column.label).strong()),
                );
            }
        });

        let row_height = 25.0;
        let scroll_request = editor.state.table_state_mut(grid_id).take_scroll_request();
        let mut scroll_area = egui::ScrollArea::vertical().id_salt(("grid-scroll", grid_id.get()));
        if let Some(row) = scroll_request {
            scroll_area = scroll_area.vertical_scroll_offset(row as f32 * row_height);
        }
        scroll_area.show_rows(ui, row_height, rows.len(), |ui, range| {
            for row_index in range {
                let row = rows[row_index];
                ui.horizontal(|ui| {
                    for (column_index, column) in columns.iter().enumerate() {
                        let address = crate::GridCellAddress {
                            row,
                            column: column.id,
                        };
                        let width = table_snapshot
                            .column_widths
                            .get(column_index)
                            .copied()
                            .unwrap_or(match column.kind {
                                crate::GridColumnKind::Validation => 240.0,
                                crate::GridColumnKind::RowId => 55.0,
                                _ => 105.0,
                            });
                        let selected_cell = selected.contains(&address);
                        let text = row_values[row_index][column_index].clone();
                        match column.kind {
                            crate::GridColumnKind::RowId => {
                                let response = ui.add_sized(
                                    [width, 20.0],
                                    egui::Button::selectable(selected_cell, text),
                                );
                                if response.clicked() {
                                    ui.memory_mut(|memory| memory.request_focus(table_focus));
                                    editor
                                        .state
                                        .table_state_mut(grid_id)
                                        .select_row(row, &columns);
                                    *command =
                                        Some(EditorCommand::SetSelection(Selection::GridRow {
                                            grid: grid_id,
                                            row,
                                        }));
                                }
                            }
                            crate::GridColumnKind::Component {
                                component,
                                dependent,
                            } if column.editable && !dependent => {
                                let invalid = editor
                                    .document
                                    .grid(grid_id)
                                    .and_then(|grid| grid.row_validation(row).ok())
                                    .is_some_and(|issues| {
                                        issues.iter().any(|issue| {
                                            matches!(
                                                issue,
                                                crate::GridValidationIssue::InvalidNumber {
                                                    column,
                                                    ..
                                                } | crate::GridValidationIssue::NonFiniteComponent {
                                                    column
                                                } | crate::GridValidationIssue::NegativeComponent {
                                                    column
                                                } if *column == component
                                            )
                                        })
                                    });
                                let mut value = text;
                                let frame = if invalid {
                                    egui::Frame::NONE.fill(egui::Color32::from_rgb(92, 30, 36))
                                } else if selected_cell {
                                    egui::Frame::NONE.fill(egui::Color32::from_rgb(45, 62, 92))
                                } else {
                                    egui::Frame::NONE
                                };
                                frame.show(ui, |ui| {
                                    let response = ui.add_sized(
                                        [width, 20.0],
                                        egui::TextEdit::singleline(&mut value),
                                    );
                                    if response.clicked() {
                                        activate_cell(
                                            ui,
                                            editor,
                                            grid_id,
                                            address,
                                            table_focus,
                                            false,
                                        );
                                    }
                                    if response.changed() {
                                        *command = Some(EditorCommand::EditGridComponent {
                                            grid: grid_id,
                                            row,
                                            component,
                                            text: value.clone(),
                                        });
                                    }
                                    if invalid {
                                        response.on_hover_text(
                                            editor
                                                .document
                                                .grid(grid_id)
                                                .and_then(|grid| grid.row_validation(row).ok())
                                                .map(|issues| format!("{issues:?}"))
                                                .unwrap_or_default(),
                                        );
                                    }
                                });
                            }
                            crate::GridColumnKind::Scalar { field } => {
                                let invalid = !text.trim().is_empty()
                                    && text
                                        .trim()
                                        .parse::<f64>()
                                        .ok()
                                        .is_none_or(|value| !value.is_finite());
                                let mut value = text;
                                let frame = if invalid {
                                    egui::Frame::NONE.fill(egui::Color32::from_rgb(92, 30, 36))
                                } else if selected_cell {
                                    egui::Frame::NONE.fill(egui::Color32::from_rgb(45, 62, 92))
                                } else {
                                    egui::Frame::NONE
                                };
                                frame.show(ui, |ui| {
                                    let response = ui.add_sized(
                                        [width, 20.0],
                                        egui::TextEdit::singleline(&mut value),
                                    );
                                    if response.clicked() {
                                        activate_cell(
                                            ui,
                                            editor,
                                            grid_id,
                                            address,
                                            table_focus,
                                            false,
                                        );
                                    }
                                    if response.changed() {
                                        *command = Some(EditorCommand::EditGridScalarText {
                                            grid: grid_id,
                                            row,
                                            field,
                                            text: value.clone(),
                                        });
                                    }
                                    if invalid {
                                        response.on_hover_text(
                                            "Invalid scalar text is retained until corrected.",
                                        );
                                    }
                                });
                            }
                            crate::GridColumnKind::Validation => {
                                let color = if text.is_empty() {
                                    egui::Color32::from_rgb(80, 180, 105)
                                } else {
                                    egui::Color32::from_rgb(235, 116, 91)
                                };
                                let response = ui.add_sized(
                                    [width, 20.0],
                                    egui::Label::new(
                                        egui::RichText::new(if text.is_empty() {
                                            "valid"
                                        } else {
                                            &text
                                        })
                                        .color(color),
                                    ),
                                );
                                if !text.is_empty() {
                                    response.on_hover_text(text);
                                }
                            }
                            crate::GridColumnKind::Component { .. } => {
                                let response = ui.add_sized(
                                    [width, 20.0],
                                    egui::Button::selectable(selected_cell, text),
                                );
                                if response.clicked() {
                                    activate_cell(ui, editor, grid_id, address, table_focus, false);
                                }
                            }
                        }
                    }
                });
            }
        });

        handle_table_keyboard(
            ui,
            editor,
            grid_id,
            TableKeyboardContext {
                rows: &rows,
                columns: &columns,
                focus: table_focus,
            },
            clipboard,
            command,
        );
    }

    fn activate_cell(
        ui: &egui::Ui,
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        address: crate::GridCellAddress,
        focus: egui::Id,
        navigation_focus: bool,
    ) {
        let extend = ui.input(|input| input.modifiers.shift);
        if navigation_focus {
            ui.memory_mut(|memory| memory.request_focus(focus));
        }
        editor.state.table_state_mut(grid).activate(address, extend);
        editor.state.select(Selection::GridCell {
            grid,
            row: address.row,
            column: address.column,
        });
    }

    fn copy_table_selection(
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
        headers: bool,
        success: &str,
    ) {
        let output = editor.document.grid(grid).and_then(|value| {
            editor
                .state
                .table_state(grid)
                .and_then(|table| table.copy(value, headers).ok())
        });
        match (clipboard.as_mut(), output) {
            (Some(clipboard), Some(table)) => match clipboard.set_text(table.to_tsv(headers)) {
                Ok(()) => editor.state.status = Some(success.to_owned()),
                Err(error) => editor.state.status = Some(error.to_string()),
            },
            (None, _) => editor.state.status = Some("System clipboard is unavailable.".to_owned()),
            (_, None) => editor.state.status = Some("Nothing is selected to copy.".to_owned()),
        }
    }

    fn copy_selected_rows(
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
    ) {
        let output = editor.document.grid(grid).and_then(|value| {
            let displayed = editor
                .state
                .table_state(grid)
                .map_or_else(|| value.row_ids(), |table| table.displayed_rows(value));
            let columns = crate::grid_columns(value)
                .into_iter()
                .filter(|column| !matches!(column.kind, crate::GridColumnKind::Validation))
                .collect::<Vec<_>>();
            let mut selected_rows = editor
                .state
                .table_state(grid)
                .map(|table| {
                    table
                        .selected_addresses(&displayed, &crate::grid_columns(value))
                        .into_iter()
                        .map(|cell| cell.row)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            selected_rows.sort();
            selected_rows.dedup();
            if selected_rows.is_empty() {
                return None;
            }
            let rows = displayed
                .into_iter()
                .filter(|row| selected_rows.contains(row))
                .map(|row| {
                    columns
                        .iter()
                        .map(|column| crate::cell_text(value, row, column).unwrap_or_default())
                        .collect()
                })
                .collect();
            Some(crate::ClipboardTable {
                headers: Some(columns.iter().map(|column| column.label.clone()).collect()),
                rows,
            })
        });
        write_clipboard(editor, clipboard, output.map(|table| table.to_tsv(true)));
    }

    fn write_clipboard(
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        text: Option<String>,
    ) {
        match (clipboard.as_mut(), text) {
            (Some(clipboard), Some(text)) => match clipboard.set_text(text) {
                Ok(()) => editor.state.status = Some("Copied TSV to clipboard.".to_owned()),
                Err(error) => editor.state.status = Some(error.to_string()),
            },
            (None, _) => editor.state.status = Some("System clipboard is unavailable.".to_owned()),
            (_, None) => editor.state.status = Some("Unable to build clipboard data.".to_owned()),
        }
    }

    fn paste_from_clipboard(
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
        transposed: bool,
        command: &mut Option<EditorCommand>,
    ) {
        let Some(text) = clipboard.as_mut().and_then(|value| value.get_text().ok()) else {
            editor.state.status = Some("System clipboard is unavailable or empty.".to_owned());
            return;
        };
        let Some(anchor) = editor
            .state
            .table_state(grid)
            .and_then(|table| table.active_cell)
        else {
            editor.state.status = Some("Select a destination cell before pasting.".to_owned());
            return;
        };
        let table = parse_clipboard_for_grid(&text, editor.document.grid(grid));
        *command = Some(EditorCommand::PasteGridCells {
            grid,
            anchor,
            clipboard: table,
            transposed,
        });
    }

    fn parse_clipboard_for_grid(
        text: &str,
        grid: Option<&crate::CompositionGrid>,
    ) -> crate::ClipboardTable {
        let first = text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .lines()
            .next()
            .unwrap_or_default()
            .split('\t')
            .map(|cell| cell.trim().to_ascii_lowercase())
            .collect::<Vec<_>>();
        let labels = grid
            .map(crate::grid_columns)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|column| {
                let mut labels = vec![column.label.to_ascii_lowercase()];
                match column.kind {
                    crate::GridColumnKind::Component { component, .. } => {
                        labels.push(["a", "b", "c", "d"][component].to_owned());
                        if component < 3 {
                            labels.push(["u", "v", "w"][component].to_owned());
                        }
                    }
                    crate::GridColumnKind::RowId => labels.push("row id".to_owned()),
                    _ => {}
                }
                labels
            })
            .collect::<Vec<_>>();
        let has_known_header = first
            .iter()
            .any(|cell| labels.iter().any(|label| label == cell));
        let has_text_header = text.lines().nth(1).is_some()
            && first.iter().any(|cell| {
                !cell.is_empty()
                    && cell.parse::<f64>().is_err()
                    && !matches!(
                        cell.as_str(),
                        "nan" | "inf" | "+inf" | "-inf" | "infinity" | "+infinity" | "-infinity"
                    )
            });
        let has_headers = has_known_header || has_text_header;
        crate::ClipboardTable::parse_tsv(text, has_headers)
    }

    struct TableKeyboardContext<'a> {
        rows: &'a [crate::GridRowId],
        columns: &'a [crate::GridColumn],
        focus: egui::Id,
    }

    fn handle_table_keyboard(
        ui: &egui::Ui,
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        table: TableKeyboardContext<'_>,
        clipboard: &mut Option<arboard::Clipboard>,
        command: &mut Option<EditorCommand>,
    ) {
        let TableKeyboardContext {
            rows,
            columns,
            focus,
        } = table;
        if !ui.memory(|memory| memory.has_focus(focus)) {
            return;
        }
        let shift = ui.input(|input| input.modifiers.shift);
        let movement = if ui.input(|input| input.key_pressed(egui::Key::ArrowLeft)) {
            Some(crate::TableMove::Left)
        } else if ui.input(|input| input.key_pressed(egui::Key::ArrowRight)) {
            Some(crate::TableMove::Right)
        } else if ui.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
            Some(crate::TableMove::Up)
        } else if ui.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
            Some(crate::TableMove::Down)
        } else if ui.input(|input| input.key_pressed(egui::Key::Tab)) {
            Some(if shift {
                crate::TableMove::Left
            } else {
                crate::TableMove::Right
            })
        } else if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            Some(if shift {
                crate::TableMove::Up
            } else {
                crate::TableMove::Down
            })
        } else {
            None
        };
        if let Some(movement) = movement {
            editor
                .state
                .table_state_mut(grid)
                .move_active(rows, columns, movement, shift);
        }
        let command_modifier = ui.input(|input| input.modifiers.command);
        if command_modifier && ui.input(|input| input.key_pressed(egui::Key::A)) {
            editor.state.table_state_mut(grid).select_all(rows, columns);
        }
        if command_modifier && ui.input(|input| input.key_pressed(egui::Key::C)) {
            copy_table_selection(editor, grid, clipboard, false, "Copied selected cells");
        }
        if command_modifier && ui.input(|input| input.key_pressed(egui::Key::V)) {
            paste_from_clipboard(editor, grid, clipboard, false, command);
        }
        if ui.input(|input| {
            input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
        }) {
            let cells = editor
                .state
                .table_state(grid)
                .map(|table| table.selected_addresses(rows, columns))
                .unwrap_or_default();
            *command = Some(EditorCommand::ClearGridCells { grid, cells });
        }
    }

    fn entry_mode_label(mode: crate::CompositionEntryMode, dimension: usize) -> String {
        match mode {
            crate::CompositionEntryMode::AllComponents => "All components".to_owned(),
            crate::CompositionEntryMode::DependentComponent(component) => {
                format!("{} dependent", component_label(component, dimension))
            }
        }
    }

    fn component_label(component: crate::Component, dimension: usize) -> &'static str {
        if dimension == 3 {
            ["u", "v", "w", "?"][component.index()]
        } else {
            ["A", "B", "C", "D"][component.index()]
        }
    }

    fn restrict_events_to_viewport(events: &mut [three_d::Event], viewport: crate::EditorViewport) {
        for event in events {
            match event {
                three_d::Event::MousePress {
                    position, handled, ..
                }
                | three_d::Event::MouseRelease {
                    position, handled, ..
                }
                | three_d::Event::MouseMotion {
                    position, handled, ..
                }
                | three_d::Event::MouseWheel {
                    position, handled, ..
                }
                | three_d::Event::PinchGesture {
                    position, handled, ..
                }
                | three_d::Event::RotationGesture {
                    position, handled, ..
                } if !viewport.contains_physical([position.x, position.y]) => {
                    *handled = true;
                }
                _ => {}
            }
        }
    }

    fn handle_3d_pick(
        events: &[three_d::Event],
        camera: &NativeCamera,
        viewport: crate::EditorViewport,
        editor: &mut TetraplotEditor,
    ) {
        let click = events.iter().find_map(|event| match event {
            three_d::Event::MousePress {
                button: three_d::MouseButton::Left,
                position,
                handled: false,
                ..
            } if viewport.contains_physical([position.x, position.y]) => Some(*position),
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
        let point_radius = editor.document.plot().scene_bounds().max_extent().max(1.0) * 0.035;
        if let Some(hit) = crate::pick_plot_series(ray, editor.document.plot(), point_radius) {
            candidates.push(hit);
        }
        for grid in editor.document.grids() {
            let Some(id) = grid.id() else {
                continue;
            };
            if let Ok(points) = editor.document.prepared_grid_points(id)
                && let Some(hit) = crate::pick_grid_points(ray, &points, point_radius)
            {
                candidates.push(hit);
            }
        }
        for chart in editor.document.plot().embedded_charts() {
            if !chart.visible() {
                continue;
            }
            let Some(id) = chart.id() else {
                continue;
            };
            let Ok(prepared) = chart.prepared(&geometry, tolerance) else {
                continue;
            };
            if let Some(hit) = crate::pick_embedded_series(ray, id, &prepared, point_radius) {
                candidates.push(hit);
            }
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
            if !section.visible() {
                continue;
            }
            let Some(id) = section.id() else {
                continue;
            };
            let Ok(prepared) = section.prepared(&geometry, tolerance) else {
                continue;
            };
            if let Some(hit) = crate::pick_section_series(ray, id, &prepared, point_radius) {
                candidates.push(hit);
            }
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
        let Some(hit) = crate::select_best_pick(ray, candidates) else {
            return;
        };
        if let Err(error) = EditorCommand::SetSelection(hit.selection.clone())
            .execute(&mut editor.document, &mut editor.state)
        {
            editor.state.status = Some(error.to_string());
            return;
        }
        if let (Some(local), Some(tetrahedral)) = (hit.local_position, hit.tetrahedral_position) {
            let owner = match hit.selection {
                Selection::Section(id) => Some(LinkedCursorOwner::Section(id)),
                Selection::SurfacePatch { chart, .. }
                | Selection::EmbeddedChart(chart)
                | Selection::BreakLine { chart, .. }
                | Selection::EmbeddedSeries { chart, .. } => {
                    Some(LinkedCursorOwner::EmbeddedChart(chart))
                }
                Selection::GridRow { grid, .. } => {
                    editor
                        .document
                        .grid(grid)
                        .and_then(|grid| match grid.coordinate_space() {
                            crate::GridCoordinateSpace::Section(id) => {
                                Some(LinkedCursorOwner::Section(id))
                            }
                            crate::GridCoordinateSpace::EmbeddedChart(id) => {
                                Some(LinkedCursorOwner::EmbeddedChart(id))
                            }
                            crate::GridCoordinateSpace::Tetrahedral => None,
                        })
                }
                _ => None,
            };
            if let Some(owner) = owner {
                editor.state.set_cursor(Some(LinkedCursor {
                    owner,
                    local_position: local,
                    tetrahedral_position: tetrahedral,
                    world_position: hit.world_position,
                    surface_triangle: hit.surface_triangle,
                    patch: hit.patch,
                }));
            }
        }
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
