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
        let mut grid_models = build_grid_models(&editor, &context);
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
        let mut highlight_models = build_highlight_models(&editor, &context);
        let mut highlight_revision = editor.state.revision();
        let mut flat_cache: Option<FlatTexture> = None;
        let started = Instant::now();
        window.render_loop(move |mut frame| {
            let mut interaction = UiOutcome::default();
            gui.update(
                &mut frame.events,
                started.elapsed().as_secs_f64() * 1000.0,
                frame.viewport,
                frame.device_pixel_ratio,
                |ui| {
                    interaction = ui_shell(ui, &mut editor, &mut clipboard, &mut flat_cache);
                },
            );
            let camera_action = matches!(
                interaction.command,
                Some(EditorCommand::FitCamera | EditorCommand::ResetCamera)
            );
            if let Some(command) = interaction.command
                && let Err(error) = command.execute(&mut editor.document, &mut editor.state)
            {
                editor.state.status = Some(error.to_string());
            }
            if camera_action {
                let source = editor.document.plot().camera();
                let fov = match source.projection() {
                    crate::Projection::Perspective {
                        vertical_fov_radians,
                    } => vertical_fov_radians.to_degrees() as f32,
                    crate::Projection::Orthographic { .. } => 45.0,
                };
                camera = NativeCamera::new_perspective(
                    frame.viewport,
                    vec3(source.position().map(|value| value as f32)),
                    vec3(source.target().map(|value| value as f32)),
                    vec3(source.up().map(|value| value as f32)),
                    three_d::degrees(fov),
                    source.near() as f32,
                    source.far() as f32,
                );
                orbit = OrbitControl::new(camera.target(), 0.01, source.far() as f32);
            }
            editor.state.clear_removed(&editor.document);
            if editor.document.revision() != model_revision {
                match crate::render::three_d_backend::models(editor.document.plot(), &context) {
                    Ok(updated) => {
                        grid_models = build_grid_models(&editor, &context);
                        models = updated;
                        model_revision = editor.document.revision();
                    }
                    Err(error) => editor.state.status = Some(error.to_string()),
                }
                if editor.state.revision() != highlight_revision {
                    highlight_models = build_highlight_models(&editor, &context);
                    highlight_revision = editor.state.revision();
                }
            }
            let editor_viewport = interaction.viewport_rect.and_then(|rect| {
                crate::EditorViewport::from_logical_rect(
                    [rect.min.x, rect.min.y, rect.max.x, rect.max.y],
                    [frame.viewport.width, frame.viewport.height],
                    frame.device_pixel_ratio,
                )
            });
            if let Some(editor_viewport) = editor_viewport {
                let physical = editor_viewport.physical_rect;
                let viewport = three_d::Viewport {
                    x: physical.x,
                    y: physical.y,
                    width: physical.width,
                    height: physical.height,
                };
                camera.set_viewport(viewport);
                let mut scene_events: Vec<_> = frame
                    .events
                    .iter()
                    .filter(|event| event_in_viewport(event, editor_viewport))
                    .cloned()
                    .collect();
                handle_3d_pick(&scene_events, &camera, &mut editor, editor_viewport);
                orbit.handle_events(&mut camera, &mut scene_events);
            }
            let background = editor.document.plot().background().clamped();
            let screen = frame.screen();
            screen.clear(ClearState::color_and_depth(
                background.red() * 0.45,
                background.green() * 0.45,
                background.blue() * 0.45,
                1.0,
                1.0,
            ));
            if let Some(editor_viewport) = editor_viewport {
                let physical = editor_viewport.physical_rect;
                let scissor = three_d::ScissorBox {
                    x: physical.x,
                    y: physical.y,
                    width: physical.width,
                    height: physical.height,
                };
                screen
                    .clear_partially(
                        scissor,
                        ClearState::color_and_depth(
                            background.red(),
                            background.green(),
                            background.blue(),
                            1.0,
                            1.0,
                        ),
                    )
                    .render_partially(
                        scissor,
                        &camera,
                        models.iter().chain(&grid_models).chain(&highlight_models),
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

    type NativeModel = three_d::Gm<three_d::Mesh, three_d::ColorMaterial>;

    fn build_grid_models(editor: &TetraplotEditor, context: &three_d::Context) -> Vec<NativeModel> {
        let points: Vec<_> = editor
            .document
            .grids()
            .filter_map(|grid| grid.id())
            .flat_map(|grid| {
                editor
                    .document
                    .prepared_grid_points(grid)
                    .unwrap_or_default()
            })
            .map(|point| point.world)
            .collect();
        crate::render::three_d_backend::point_cloud_model(
            context,
            &points,
            0.022,
            crate::Color::rgb(0.12, 0.58, 0.95),
        )
        .into_iter()
        .collect()
    }

    fn build_highlight_models(
        editor: &TetraplotEditor,
        context: &three_d::Context,
    ) -> Vec<NativeModel> {
        let mut result = Vec::new();
        if let Selection::GridRow { grid, row } = editor.state.selection
            && let Ok(points) = editor.document.prepared_grid_points(grid)
            && let Some(point) = points.iter().find(|point| point.row_id == row)
            && let Some(model) = crate::render::three_d_backend::point_cloud_model(
                context,
                &[point.world],
                0.045,
                crate::Color::rgb(1.0, 0.15, 0.75),
            )
        {
            result.push(model);
        }
        if let Some(cursor) = &editor.state.linked_cursor
            && let Some(model) = crate::render::three_d_backend::point_cloud_model(
                context,
                &[cursor.world_position.map(|value| value as f32)],
                0.032,
                crate::RED,
            )
        {
            result.push(model);
        }
        match editor.state.selection.clone() {
            Selection::EmbeddedChart(chart) => {
                if let Some(prepared) =
                    editor
                        .document
                        .plot()
                        .embedded_chart(chart)
                        .and_then(|chart| {
                            chart
                                .prepared(
                                    &editor.document.plot().geometry(),
                                    editor.document.plot().tolerance(),
                                )
                                .ok()
                        })
                {
                    append_surface_highlight(&mut result, context, &prepared, None);
                }
            }
            Selection::SurfacePatch { chart, patch } => {
                if let Some(prepared) =
                    editor
                        .document
                        .plot()
                        .embedded_chart(chart)
                        .and_then(|chart| {
                            chart
                                .prepared(
                                    &editor.document.plot().geometry(),
                                    editor.document.plot().tolerance(),
                                )
                                .ok()
                        })
                {
                    append_surface_highlight(&mut result, context, &prepared, Some(patch));
                }
            }
            Selection::BreakLine { chart, line } => {
                if let Some(prepared) =
                    editor
                        .document
                        .plot()
                        .embedded_chart(chart)
                        .and_then(|chart| {
                            chart
                                .prepared(
                                    &editor.document.plot().geometry(),
                                    editor.document.plot().tolerance(),
                                )
                                .ok()
                        })
                    && let Some(line) = prepared.break_lines.iter().find(|item| item.id == line)
                {
                    let segments: Vec<_> = line
                        .line
                        .segments
                        .iter()
                        .map(|segment| segment.world)
                        .collect();
                    if let Some(model) = crate::render::three_d_backend::line_segments_model(
                        context,
                        &segments,
                        0.018,
                        crate::Color::rgb(1.0, 0.1, 0.75),
                    ) {
                        result.push(model);
                    }
                }
            }
            Selection::EmbeddedSeries { chart, series } => {
                if let Some(prepared) =
                    editor
                        .document
                        .plot()
                        .embedded_chart(chart)
                        .and_then(|chart| {
                            chart
                                .prepared(
                                    &editor.document.plot().geometry(),
                                    editor.document.plot().tolerance(),
                                )
                                .ok()
                        })
                {
                    append_series_highlight(&mut result, context, &prepared, series);
                }
            }
            Selection::Section(section) => {
                if let Some(prepared) =
                    editor.document.plot().section(section).and_then(|section| {
                        section
                            .prepared(
                                &editor.document.plot().geometry(),
                                editor.document.plot().tolerance(),
                            )
                            .ok()
                    })
                {
                    append_surface_highlight(&mut result, context, &prepared, None);
                }
            }
            Selection::SectionSeries { section, series } => {
                if let Some(prepared) =
                    editor.document.plot().section(section).and_then(|section| {
                        section
                            .prepared(
                                &editor.document.plot().geometry(),
                                editor.document.plot().tolerance(),
                            )
                            .ok()
                    })
                {
                    append_series_highlight(&mut result, context, &prepared, series);
                }
            }
            _ => {}
        }
        result
    }

    fn append_surface_highlight(
        result: &mut Vec<NativeModel>,
        context: &three_d::Context,
        prepared: &crate::PreparedEmbeddedChart,
        patch: Option<crate::SurfacePatchId>,
    ) {
        let triangles: Vec<_> = prepared
            .surface
            .triangles
            .iter()
            .filter(|triangle| patch.is_none_or(|patch| triangle.patch == patch))
            .map(|triangle| {
                triangle
                    .indices
                    .map(|index| prepared.surface.vertices[index as usize].world)
            })
            .collect();
        if let Some(model) = crate::render::three_d_backend::triangles_model(
            context,
            &triangles,
            if patch.is_some() {
                crate::Color::rgb(1.0, 0.45, 0.05).with_alpha(0.24)
            } else {
                crate::Color::rgb(0.95, 0.85, 0.1).with_alpha(0.16)
            },
        ) {
            result.push(model);
        }
    }

    fn append_series_highlight(
        result: &mut Vec<NativeModel>,
        context: &three_d::Context,
        prepared: &crate::PreparedEmbeddedChart,
        series: crate::SectionSeriesId,
    ) {
        if let Some(points) = prepared
            .points
            .iter()
            .find(|candidate| candidate.series_id == series)
        {
            let points: Vec<_> = points.points.iter().map(|point| point.world).collect();
            if let Some(model) = crate::render::three_d_backend::point_cloud_model(
                context,
                &points,
                0.042,
                crate::Color::rgb(1.0, 0.1, 0.75),
            ) {
                result.push(model);
            }
        }
        let segments: Vec<_> = prepared
            .lines
            .iter()
            .filter(|line| line.series_id == Some(series))
            .flat_map(|line| line.segments.iter().map(|segment| segment.world))
            .collect();
        if let Some(model) = crate::render::three_d_backend::line_segments_model(
            context,
            &segments,
            0.014,
            crate::Color::rgb(1.0, 0.1, 0.75),
        ) {
            result.push(model);
        }
    }

    struct FlatTexture {
        key: (FlatViewTarget, u64, u64, u32, u32),
        image: FlatChartImage,
        texture: egui::TextureHandle,
    }

    #[derive(Default)]
    struct UiOutcome {
        command: Option<EditorCommand>,
        viewport_rect: Option<egui::Rect>,
    }
    fn ui_shell(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        flat_cache: &mut Option<FlatTexture>,
    ) -> UiOutcome {
        let mut command = None;
        egui::Panel::top("editor-menu").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("tetraplot editor").strong());
                if ui.button("Fit scene").clicked() {
                    command = Some(EditorCommand::FitCamera)
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
                    .on_hover_text("Enable the image-export feature to save PNG files.");
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
        let viewport_rect = Some(
            egui::CentralPanel::default()
                .show_inside(ui, |ui| {
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
                })
                .response
                .rect,
        );
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
        UiOutcome {
            command,
            viewport_rect,
        }
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
            Selection::Frame => {
                let frame = editor.document.plot().frame();
                let mut edge_color = to_egui_color(frame.edge_color());
                let mut edge_width = frame.edge_width();
                let mut faces = frame.faces();
                let mut face_color = to_egui_color(frame.face_color());
                let mut background = to_egui_color(editor.document.plot().background());
                let changed = ui.color_edit_button_srgba(&mut edge_color).changed()
                    | ui.add(egui::Slider::new(&mut edge_width, 0.2..=8.0).text("edge width"))
                        .changed()
                    | ui.checkbox(&mut faces, "show faces").changed()
                    | ui.color_edit_button_srgba(&mut face_color).changed()
                    | ui.color_edit_button_srgba(&mut background).changed();
                ui.small("Edge colour ? face colour ? background");
                if changed {
                    *command = Some(EditorCommand::SetFrameProperties {
                        edge_color: from_egui_color(edge_color),
                        edge_width,
                        faces,
                        face_color: from_egui_color(face_color),
                        background: from_egui_color(background),
                    });
                }
            }
            Selection::Section(id) => {
                if let Some(section) = editor.document.plot().section(id) {
                    let mut name = section.label().unwrap_or("Planar section").to_owned();
                    if ui.text_edit_singleline(&mut name).changed() {
                        *command = Some(EditorCommand::SetName {
                            target: Selection::Section(id),
                            name,
                        });
                    }
                    let mut visible = section.visible();
                    if ui.checkbox(&mut visible, "visible").changed() {
                        *command = Some(EditorCommand::SetVisibility {
                            target: Selection::Section(id),
                            visible,
                        });
                    }
                    let mut style = section.style();
                    let mut style_changed = ui
                        .add(
                            egui::Slider::new(&mut style.fill_opacity, 0.0..=1.0)
                                .text("fill opacity"),
                        )
                        .changed();
                    style_changed |= ui
                        .checkbox(&mut style.boundary_visible, "boundary")
                        .changed();
                    style_changed |= ui.checkbox(&mut style.grid.visible, "local grid").changed();
                    if style_changed {
                        *command = Some(EditorCommand::SetChartStyle {
                            target: Selection::Section(id),
                            style,
                        });
                    }
                    if let crate::SectionPlane::ConstantComponent { component, value } =
                        section.plane()
                    {
                        let mut value = value;
                        if ui
                            .add(
                                egui::Slider::new(&mut value, 0.0..=1.0)
                                    .text(format!("{component:?} value")),
                            )
                            .changed()
                        {
                            *command = Some(EditorCommand::SetSectionValue { section: id, value });
                        }
                    } else {
                        ui.label(format!("{:?}", section.plane()));
                    }
                    if ui.button("Open flat view").clicked() {
                        *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::Section(id)));
                    }
                    if ui.button("Remove section").clicked() {
                        *command = Some(EditorCommand::RemoveSection(id));
                    }
                }
            }
            Selection::EmbeddedChart(id) => {
                if let Some(chart) = editor.document.plot().embedded_chart(id) {
                    let mut name = chart.label().unwrap_or("Embedded chart").to_owned();
                    if ui.text_edit_singleline(&mut name).changed() {
                        *command = Some(EditorCommand::SetName {
                            target: Selection::EmbeddedChart(id),
                            name,
                        });
                    }
                    let mut visible = chart.visible();
                    if ui.checkbox(&mut visible, "visible").changed() {
                        *command = Some(EditorCommand::SetVisibility {
                            target: Selection::EmbeddedChart(id),
                            visible,
                        });
                    }
                    let mut style = chart.style();
                    let mut changed = ui
                        .add(
                            egui::Slider::new(&mut style.fill_opacity, 0.0..=1.0)
                                .text("fill opacity"),
                        )
                        .changed();
                    changed |= ui
                        .checkbox(&mut style.boundary_visible, "boundary")
                        .changed();
                    changed |= ui.checkbox(&mut style.grid.visible, "local grid").changed();
                    changed |= ui
                        .checkbox(&mut style.two_sided, "two-sided surface")
                        .changed();
                    egui::ComboBox::from_id_salt(("normals", id.get()))
                        .selected_text(format!("{:?}", style.normal_mode))
                        .show_ui(ui, |ui| {
                            for mode in [
                                crate::SurfaceNormalMode::Flat,
                                crate::SurfaceNormalMode::SmoothWithinPatches,
                            ] {
                                if ui
                                    .selectable_label(
                                        style.normal_mode == mode,
                                        format!("{mode:?}"),
                                    )
                                    .clicked()
                                {
                                    style.normal_mode = mode;
                                    changed = true;
                                }
                            }
                        });
                    if changed {
                        *command = Some(EditorCommand::SetChartStyle {
                            target: Selection::EmbeddedChart(id),
                            style,
                        });
                    }
                    if ui.button("Open flat view").clicked() {
                        *command = Some(EditorCommand::OpenFlatView(
                            FlatViewTarget::EmbeddedChart(id),
                        ));
                    }
                    if ui.button("Remove chart").clicked() {
                        *command = Some(EditorCommand::RemoveEmbeddedChart(id));
                    }
                }
            }
            Selection::SurfacePatch { chart, patch } => {
                ui.label(format!("Patch {} in chart {}", patch.get(), chart.get()));
                ui.small(
                    "Selection highlighting is renderer-only; source patch style is unchanged.",
                );
            }
            Selection::BreakLine { chart, line } => {
                if let Some(break_line) =
                    editor
                        .document
                        .plot()
                        .embedded_chart(chart)
                        .and_then(|chart| match chart.embedding() {
                            crate::ChartEmbedding::Triangulated(embedding) => embedding
                                .break_lines()
                                .iter()
                                .find(|item| item.id() == line),
                            crate::ChartEmbedding::Planar(_) => None,
                        })
                {
                    let mut kind = break_line.kind();
                    let mut style = break_line.style();
                    let mut changed = ui.checkbox(&mut style.visible, "visible").changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut style.width, 0.2..=10.0).text("width"))
                        .changed();
                    let mut color = to_egui_color(style.color);
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        style.color = from_egui_color(color);
                        changed = true;
                    }
                    egui::ComboBox::from_id_salt(("break-kind", chart.get(), line.get()))
                        .selected_text(format!("{kind:?}"))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                crate::BreakLineKind::Crease,
                                crate::BreakLineKind::Univariant,
                                crate::BreakLineKind::PhaseBoundary,
                                crate::BreakLineKind::UserDefined,
                            ] {
                                if ui
                                    .selectable_label(kind == candidate, format!("{candidate:?}"))
                                    .clicked()
                                {
                                    kind = candidate;
                                    changed = true;
                                }
                            }
                        });
                    let adjacent = break_line.adjacent_patches();
                    ui.label(format!(
                        "Adjacent patches: {} and {}",
                        adjacent[0].get(),
                        adjacent[1].get()
                    ));
                    if changed {
                        *command = Some(EditorCommand::SetBreakLineProperties {
                            chart,
                            line,
                            kind,
                            style,
                        });
                    }
                }
            }
            Selection::Grid(id) => {
                if let Some(grid) = editor.document.grid(id) {
                    let mut name = grid.name().to_owned();
                    if ui.text_edit_singleline(&mut name).changed() {
                        *command = Some(EditorCommand::RenameGrid { grid: id, name });
                    }
                    ui.label(format!(
                        "{} ? {:?} ? {} rows",
                        if grid.is_regular() {
                            "Regular"
                        } else {
                            "Irregular"
                        },
                        grid.coordinate_space(),
                        grid.rows_len()
                    ));
                    ui.label(format!(
                        "Entry: {:?} ? duplicates: {:?}",
                        grid.entry_mode(),
                        grid.duplicate_policy()
                    ));
                    if ui.button("Open data table").clicked() {
                        *command = Some(EditorCommand::OpenDataGrid(id));
                    }
                    if ui.button("Remove grid").clicked() {
                        *command = Some(EditorCommand::RemoveGrid { grid: id });
                    }
                }
            }
            Selection::ScalarField { grid, field } => {
                if let Some(field_data) = editor
                    .document
                    .grid(grid)
                    .and_then(|grid| grid.fields().iter().find(|item| item.id() == field))
                {
                    let mut name = field_data.name().to_owned();
                    if ui.text_edit_singleline(&mut name).changed() {
                        *command = Some(EditorCommand::RenameScalarField { grid, field, name });
                    }
                    let mut units = field_data.units().unwrap_or_default().to_owned();
                    if ui.text_edit_singleline(&mut units).changed() {
                        *command = Some(EditorCommand::SetScalarUnits {
                            grid,
                            field,
                            units: (!units.trim().is_empty()).then_some(units),
                        });
                    }
                    let mut precision = field_data.precision();
                    if ui
                        .add(egui::Slider::new(&mut precision, 0..=15).text("precision"))
                        .changed()
                    {
                        *command = Some(EditorCommand::SetScalarPrecision {
                            grid,
                            field,
                            precision,
                        });
                    }
                    if ui.button("Clear values").clicked() {
                        *command = Some(EditorCommand::ClearScalarField { grid, field });
                    }
                    if ui.button("Remove field").clicked() {
                        *command = Some(EditorCommand::RemoveScalarField { grid, field });
                    }
                }
            }
            Selection::GridRow { grid, row } => {
                ui.label(format!("Grid {} row {}", grid.get(), row.get()));
                if let Some(source) = editor.document.grid(grid) {
                    if let Ok(Some(coordinate)) = source.row_coordinate(row) {
                        ui.label(format!("Coordinate: {:?}", coordinate.as_values()));
                    }
                    if let Ok(values) = source.scalar_values(row) {
                        for (field, value) in source.fields().iter().zip(values) {
                            ui.label(format!(
                                "{}: {}",
                                field.name(),
                                value
                                    .map(|value| format!("{value:.6}"))
                                    .unwrap_or_else(|| "missing".to_owned())
                            ));
                        }
                    }
                    if let Ok(text) = source.validation_text(row) {
                        ui.label(format!("Validation: {text}"));
                    }
                }
            }
            _ => {
                ui.label("Properties are available for this selection.");
            }
        }
    }

    fn to_egui_color(color: crate::Color) -> egui::Color32 {
        egui::Color32::from_rgba_unmultiplied(
            (color.red().clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.green().clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.blue().clamp(0.0, 1.0) * 255.0).round() as u8,
            (color.alpha().clamp(0.0, 1.0) * 255.0).round() as u8,
        )
    }

    fn from_egui_color(color: egui::Color32) -> crate::Color {
        crate::Color::new(
            f32::from(color.r()) / 255.0,
            f32::from(color.g()) / 255.0,
            f32::from(color.b()) / 255.0,
            f32::from(color.a()) / 255.0,
        )
    }

    fn lower_panel(
        ui: &mut egui::Ui,
        editor: &mut TetraplotEditor,
        clipboard: &mut Option<arboard::Clipboard>,
        flat_cache: &mut Option<FlatTexture>,
        command: &mut Option<EditorCommand>,
    ) {
        let showing_data = matches!(
            editor.state.selection,
            Selection::Grid(_)
                | Selection::GridRow { .. }
                | Selection::GridCell { .. }
                | Selection::ScalarField { .. }
        ) && editor.state.active_grid.is_some();
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    !showing_data && !matches!(editor.state.flat_view, FlatViewTarget::None),
                    "Flat ternary",
                )
                .clicked()
            {
                *command = Some(EditorCommand::OpenFlatView(FlatViewTarget::FollowSelection));
            }
            if ui.selectable_label(showing_data, "Data table").clicked()
                && let Some(grid) = editor.state.active_grid
            {
                *command = Some(EditorCommand::OpenDataGrid(grid));
            }
        });
        ui.separator();
        if !showing_data && !matches!(editor.state.flat_view, FlatViewTarget::None) {
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
            ui.label("Select a planar section or embedded chart to open its local ternary view.");
            return;
        };
        let size = ui.available_size();
        let width = size.x.clamp(200.0, 900.0) as u32;
        let height = size.y.clamp(180.0, 500.0) as u32;
        let grid_points = flat_grid_points(editor, target);
        let selected_point = match editor.state.selection {
            Selection::GridRow { grid, row } => grid_points
                .iter()
                .find(|point| point.0 == grid && point.1 == row)
                .map(|point| point.2),
            _ => None,
        };
        let key = (
            target,
            flat_target_revision(editor, target),
            flat_grid_revision(editor, target) ^ editor.state.revision(),
            width,
            height,
        );
        let stale = cache.as_ref().is_none_or(|cache| cache.key != key);
        if stale {
            let selected_series = match (target, editor.state.selection.clone()) {
                (FlatViewTarget::Section(target), Selection::SectionSeries { section, series })
                    if target == section =>
                {
                    Some(series)
                }
                (
                    FlatViewTarget::EmbeddedChart(target),
                    Selection::EmbeddedSeries { chart, series },
                ) if target == chart => Some(series),
                _ => None,
            };
            let selected_break = match (target, editor.state.selection.clone()) {
                (FlatViewTarget::EmbeddedChart(target), Selection::BreakLine { chart, line })
                    if target == chart =>
                {
                    Some(line)
                }
                _ => None,
            };
            let result = editor
                .document
                .plot()
                .flat_chart(target)
                .and_then(|mut chart| {
                    let options = chart.options_mut();
                    options.grid_points = grid_points.iter().map(|point| point.2).collect();
                    options.selected_points = selected_point.into_iter().collect();
                    options.selected_series = selected_series;
                    options.selected_break = selected_break;
                    chart.render((width, height))
                });
            match result {
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
            .interact(egui::Sense::click());
        let pointer_pixel = response.hover_pos().map(|pointer| {
            let scale_x = width as f32 / response.rect.width();
            let scale_y = height as f32 / response.rect.height();
            [
                (pointer.x - response.rect.min.x) * scale_x,
                (pointer.y - response.rect.min.y) * scale_y,
            ]
        });
        if response.hovered() {
            if let Some(pixel) = pointer_pixel
                && let Some(local) = cache.image.local_at_pixel(pixel)
            {
                set_linked_cursor(editor, target, local);
            }
        } else {
            editor.state.clear_cursor();
        }
        if response.clicked()
            && let Some(pixel) = pointer_pixel
            && let Some((grid, row, _)) = grid_points
                .iter()
                .filter_map(|point| {
                    let candidate = cache.image.pixel_at_local(point.2)?;
                    let distance =
                        (candidate[0] - pixel[0]).powi(2) + (candidate[1] - pixel[1]).powi(2);
                    Some((distance, point))
                })
                .filter(|(distance, _)| *distance <= 144.0)
                .min_by(|left, right| left.0.total_cmp(&right.0))
                .map(|(_, point)| *point)
        {
            *command = Some(EditorCommand::SetSelection(Selection::GridRow {
                grid,
                row,
            }));
        }
        if let Some(cursor) = &editor.state.linked_cursor
            && cursor_matches_target(&cursor.owner, target)
            && let Some(pixel) = cache.image.pixel_at_local(cursor.local_position)
        {
            let screen = egui::pos2(
                response.rect.min.x + pixel[0] * response.rect.width() / width as f32,
                response.rect.min.y + pixel[1] * response.rect.height() / height as f32,
            );
            ui.painter()
                .circle_stroke(screen, 7.0, egui::Stroke::new(2.0_f32, egui::Color32::RED));
        }
    }

    fn cursor_matches_target(owner: &LinkedCursorOwner, target: FlatViewTarget) -> bool {
        matches!(
            (owner, target),
            (LinkedCursorOwner::Section(left), FlatViewTarget::Section(right)) if *left == right
        ) || matches!(
            (owner, target),
            (
                LinkedCursorOwner::EmbeddedChart(left),
                FlatViewTarget::EmbeddedChart(right)
            ) if *left == right
        )
    }

    fn flat_grid_points(
        editor: &TetraplotEditor,
        target: FlatViewTarget,
    ) -> Vec<(
        crate::CompositionGridId,
        crate::GridRowId,
        crate::TernaryPoint,
    )> {
        editor
            .document
            .grids()
            .filter(|grid| {
                matches!(
                    (grid.coordinate_space(), target),
                    (
                        crate::GridCoordinateSpace::Section(left),
                        FlatViewTarget::Section(right)
                    ) if left == right
                ) || matches!(
                    (grid.coordinate_space(), target),
                    (
                        crate::GridCoordinateSpace::EmbeddedChart(left),
                        FlatViewTarget::EmbeddedChart(right)
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
                    .filter_map(move |point| point.local.map(|local| (grid, point.row_id, local)))
            })
            .collect()
    }

    fn flat_grid_revision(editor: &TetraplotEditor, target: FlatViewTarget) -> u64 {
        editor
            .document
            .grids()
            .filter(|grid| {
                matches!(
                    (grid.coordinate_space(), target),
                    (
                        crate::GridCoordinateSpace::Section(left),
                        FlatViewTarget::Section(right)
                    ) if left == right
                ) || matches!(
                    (grid.coordinate_space(), target),
                    (
                        crate::GridCoordinateSpace::EmbeddedChart(left),
                        FlatViewTarget::EmbeddedChart(right)
                    ) if left == right
                )
            })
            .fold(0u64, |revision, grid| {
                revision
                    .wrapping_mul(37)
                    .wrapping_add(grid.coordinate_revision())
                    .wrapping_add(grid.structure_revision().rotate_left(17))
            })
    }

    fn flat_target_revision(editor: &TetraplotEditor, target: FlatViewTarget) -> u64 {
        match target {
            FlatViewTarget::Section(id) => editor
                .document
                .plot()
                .section(id)
                .map(|section| section.revision())
                .unwrap_or_default(),
            FlatViewTarget::EmbeddedChart(id) => editor
                .document
                .plot()
                .embedded_chart(id)
                .map(|chart| {
                    let embedding = chart.embedding().revision();
                    chart
                        .diagram()
                        .revision()
                        .wrapping_add(embedding.geometry.rotate_left(7))
                        .wrapping_add(embedding.topology.rotate_left(13))
                        .wrapping_add(embedding.breaks.rotate_left(19))
                        .wrapping_add(embedding.style.rotate_left(23))
                        .wrapping_add(u64::from(chart.style().fill_opacity.to_bits()))
                })
                .unwrap_or_default(),
            FlatViewTarget::None | FlatViewTarget::FollowSelection => 0,
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
        editor.state.set_linked_cursor(cursor);
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
        let rows = grid.row_ids();
        let columns = grid.columns();
        let name = grid.name().to_owned();
        let is_regular = grid.is_regular();
        let component_count = grid.component_count();
        let entry_mode = grid.entry_mode();
        let duplicate_policy = grid.duplicate_policy();
        let validation = grid.validation_summary();
        let fields: Vec<_> = grid
            .fields()
            .iter()
            .map(|field| {
                (
                    field.id(),
                    field.name().to_owned(),
                    field.units().map(str::to_owned),
                    field.precision(),
                )
            })
            .collect();
        let table =
            editor.state.tables.entry(grid_id).or_insert_with(|| {
                crate::DataTableState::new(grid_id, rows.clone(), columns.clone())
            });
        table.refresh(rows.clone(), columns.clone());

        let selected_cells = table.selected_cells();
        let active_cell = table.active_cell;
        let selected_rows = table.selected_rows();
        let selection_bounds = table.selection_bounds();
        let selection_anchor = table.selection.map(|range| range.anchor);
        let selected_copy = grid.selected_table(table, false).ok();
        let selected_copy_headers = grid.selected_table(table, true).ok();
        let compositions = grid
            .table(&GridExportOptions {
                scalar_fields: Vec::new(),
                ..GridExportOptions::default()
            })
            .ok();
        let full_table = grid.table(&GridExportOptions::default()).ok();
        let mut displayed = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut values = Vec::with_capacity(columns.len());
            for column in &columns {
                let address = crate::GridCellAddress {
                    row: *row,
                    column: column.id,
                };
                let value = table
                    .edit_buffer
                    .as_ref()
                    .filter(|edit| edit.address == address)
                    .map(|edit| edit.text.clone())
                    .or_else(|| table.invalid_text(address).map(str::to_owned))
                    .unwrap_or_else(|| grid.cell_text(*row, column.id).unwrap_or_default());
                values.push(value);
            }
            displayed.push(values);
        }

        ui.horizontal_wrapped(|ui| {
            ui.heading(name);
            ui.label(format!(
                "{} valid ? {} incomplete ? {} invalid ? {} warning(s)",
                validation.valid, validation.incomplete, validation.invalid, validation.warnings
            ));
            if !is_regular {
                egui::ComboBox::from_id_salt(("entry-mode", grid_id.get()))
                    .selected_text(match entry_mode {
                        crate::CompositionEntryMode::AllComponents => "All components".to_owned(),
                        crate::CompositionEntryMode::DependentComponent(component) => {
                            format!("{component:?} dependent")
                        }
                    })
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                entry_mode == crate::CompositionEntryMode::AllComponents,
                                "All components",
                            )
                            .clicked()
                        {
                            *command = Some(EditorCommand::SetEntryMode {
                                grid: grid_id,
                                mode: crate::CompositionEntryMode::AllComponents,
                            });
                        }
                        for component in crate::Component::ALL.into_iter().take(component_count) {
                            let mode = crate::CompositionEntryMode::DependentComponent(component);
                            if ui
                                .selectable_label(
                                    entry_mode == mode,
                                    format!("{component:?} dependent"),
                                )
                                .clicked()
                            {
                                *command = Some(EditorCommand::SetEntryMode {
                                    grid: grid_id,
                                    mode,
                                });
                            }
                        }
                    });
                egui::ComboBox::from_id_salt(("duplicates", grid_id.get()))
                    .selected_text(format!("Duplicates: {duplicate_policy:?}"))
                    .show_ui(ui, |ui| {
                        for policy in [
                            crate::DuplicateCompositionPolicy::Allow,
                            crate::DuplicateCompositionPolicy::Warn,
                            crate::DuplicateCompositionPolicy::Reject,
                        ] {
                            if ui
                                .selectable_label(duplicate_policy == policy, format!("{policy:?}"))
                                .clicked()
                            {
                                *command = Some(EditorCommand::SetDuplicatePolicy {
                                    grid: grid_id,
                                    policy,
                                });
                            }
                        }
                    });
            }
        });

        ui.horizontal_wrapped(|ui| {
            if ui.button("Copy").clicked() {
                copy_table_to_clipboard(
                    clipboard,
                    selected_copy.as_ref(),
                    false,
                    &mut editor.state.status,
                );
            }
            if ui.button("Copy with headers").clicked() {
                copy_table_to_clipboard(
                    clipboard,
                    selected_copy_headers.as_ref(),
                    true,
                    &mut editor.state.status,
                );
            }
            if ui.button("Copy compositions").clicked() {
                copy_table_to_clipboard(
                    clipboard,
                    compositions.as_ref(),
                    true,
                    &mut editor.state.status,
                );
            }
            if ui.button("Copy full table").clicked() {
                copy_table_to_clipboard(
                    clipboard,
                    full_table.as_ref(),
                    true,
                    &mut editor.state.status,
                );
            }
            if ui.button("Paste").clicked() {
                paste_from_clipboard(clipboard, editor, grid_id, active_cell, false, command);
            }
            if ui.button("Paste transposed").clicked() {
                paste_from_clipboard(clipboard, editor, grid_id, active_cell, true, command);
            }
            if ui.button("Clear selected cells").clicked() && !selected_cells.is_empty() {
                *command = Some(EditorCommand::ClearGridCells {
                    grid: grid_id,
                    cells: selected_cells.clone(),
                });
            }
            if ui.button("Fill down").clicked()
                && let Some(mut source) = selected_copy.clone()
                && let Some((row_start, row_end, _, _)) = selection_bounds
                && let Some(first) = source.rows.first().cloned()
                && let Some(anchor) = selection_anchor
            {
                source.headers = None;
                source.rows = vec![first; row_end - row_start + 1];
                *command = Some(EditorCommand::PasteGridCells {
                    grid: grid_id,
                    anchor,
                    clipboard: source,
                    transpose: false,
                });
            }
            if !is_regular {
                if ui.button("Insert row").clicked() {
                    *command = Some(EditorCommand::InsertGridRow {
                        grid: grid_id,
                        before: active_cell.map(|cell| cell.row),
                    });
                }
                if ui.button("Append row").clicked() {
                    *command = Some(EditorCommand::AppendGridRow { grid: grid_id });
                }
                if ui.button("Delete selected rows").clicked() && !selected_rows.is_empty() {
                    *command = Some(EditorCommand::DeleteGridRows {
                        grid: grid_id,
                        rows: selected_rows.clone(),
                    });
                }
                if ui.button("Append clipboard").clicked() {
                    match clipboard
                        .as_mut()
                        .and_then(|clipboard| clipboard.get_text().ok())
                    {
                        Some(text) => {
                            *command = Some(EditorCommand::AppendGridClipboard {
                                grid: grid_id,
                                clipboard: crate::ClipboardTable::parse_tsv_auto(&text),
                            });
                        }
                        None => {
                            editor.state.status = Some("Clipboard text is unavailable.".to_owned());
                        }
                    }
                }
            }
            if ui.button("Add scalar field").clicked() {
                *command = Some(EditorCommand::AddScalarField {
                    grid: grid_id,
                    name: format!("Field {}", fields.len() + 1),
                });
            }
        });

        ui.separator();
        egui::ScrollArea::both().show(ui, |ui| {
            ui.horizontal(|ui| {
                for column in &columns {
                    let text = if matches!(
                        column.kind,
                        crate::GridColumnKind::Component {
                            dependent: true,
                            ..
                        } | crate::GridColumnKind::LocalComponent {
                            dependent: true,
                            ..
                        }
                    ) {
                        format!("{} (dependent)", column.label)
                    } else {
                        column.label.clone()
                    };
                    let response = ui.add_sized(
                        [column_width(column), 22.0],
                        egui::Label::new(egui::RichText::new(text).strong())
                            .sense(egui::Sense::click()),
                    );
                    if response.clicked()
                        && let crate::GridColumnKind::Scalar { field } = column.kind
                    {
                        *command = Some(EditorCommand::SetSelection(Selection::ScalarField {
                            grid: grid_id,
                            field,
                        }));
                    }
                }
            });
            for (row_index, row) in rows.iter().copied().enumerate() {
                ui.horizontal(|ui| {
                    for (column_index, column) in columns.iter().enumerate() {
                        let address = crate::GridCellAddress {
                            row,
                            column: column.id,
                        };
                        let selected = selected_cells.contains(&address);
                        let invalid = editor
                            .state
                            .tables
                            .get(&grid_id)
                            .and_then(|table| table.invalid_text(address))
                            .is_some();
                        let fill = if invalid {
                            egui::Color32::from_rgb(95, 28, 34)
                        } else if selected {
                            egui::Color32::from_rgb(42, 66, 94)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let response = egui::Frame::NONE
                            .fill(fill)
                            .show(ui, |ui| {
                                let text = &mut displayed[row_index][column_index];
                                if column.editable {
                                    ui.add_sized(
                                        [column_width(column), 22.0],
                                        egui::TextEdit::singleline(text)
                                            .desired_width(column_width(column)),
                                    )
                                } else {
                                    let color = match column.kind {
                                        crate::GridColumnKind::Validation if text != "valid" => {
                                            egui::Color32::LIGHT_RED
                                        }
                                        crate::GridColumnKind::Component {
                                            dependent: true,
                                            ..
                                        }
                                        | crate::GridColumnKind::LocalComponent {
                                            dependent: true,
                                            ..
                                        } => egui::Color32::LIGHT_BLUE,
                                        _ => egui::Color32::LIGHT_GRAY,
                                    };
                                    ui.add_sized(
                                        [column_width(column), 22.0],
                                        egui::Label::new(
                                            egui::RichText::new(text.clone()).color(color),
                                        )
                                        .sense(egui::Sense::click()),
                                    )
                                }
                            })
                            .inner;
                        if response.clicked() || response.gained_focus() {
                            if let Some(table) = editor.state.tables.get_mut(&grid_id) {
                                table.activate(address, ui.input(|input| input.modifiers.shift));
                            }
                            *command = Some(EditorCommand::SetSelection(Selection::GridRow {
                                grid: grid_id,
                                row,
                            }));
                        }
                        if column.editable
                            && response.changed()
                            && let Some(table) = editor.state.tables.get_mut(&grid_id)
                        {
                            table.edit_buffer = Some(crate::CellEditBuffer::new(
                                address,
                                response_text(&displayed[row_index][column_index]),
                            ));
                        }
                        if column.editable && response.lost_focus() {
                            let text = editor
                                .state
                                .tables
                                .get_mut(&grid_id)
                                .and_then(|table| table.edit_buffer.take())
                                .filter(|edit| edit.address == address)
                                .map(|edit| edit.text)
                                .unwrap_or_else(|| displayed[row_index][column_index].clone());
                            *command = Some(EditorCommand::EditGridCellText {
                                grid: grid_id,
                                address,
                                text,
                            });
                        }
                        if matches!(column.kind, crate::GridColumnKind::Validation) {
                            response.on_hover_text(displayed[row_index][column_index].clone());
                        }
                    }
                });
            }
        });

        if !ui.ctx().egui_wants_keyboard_input() {
            handle_table_keyboard(
                ui,
                editor,
                grid_id,
                clipboard,
                selected_copy.as_ref(),
                command,
            );
        }
    }

    fn column_width(column: &crate::GridColumn) -> f32 {
        match column.kind {
            crate::GridColumnKind::RowId => 64.0,
            crate::GridColumnKind::Validation => 180.0,
            _ => 108.0,
        }
    }

    fn response_text(text: &str) -> String {
        text.to_owned()
    }

    fn copy_table_to_clipboard(
        clipboard: &mut Option<arboard::Clipboard>,
        table: Option<&crate::ClipboardTable>,
        include_headers: bool,
        status: &mut Option<String>,
    ) {
        let Some(table) = table else {
            *status = Some("No table cells are selected.".to_owned());
            return;
        };
        match clipboard {
            Some(clipboard) => match clipboard.set_text(table.to_tsv(include_headers)) {
                Ok(()) => {
                    *status = Some(format!("Copied {} row(s).", table.rows.len()));
                }
                Err(error) => *status = Some(error.to_string()),
            },
            None => *status = Some("System clipboard is unavailable.".to_owned()),
        }
    }

    fn paste_from_clipboard(
        clipboard: &mut Option<arboard::Clipboard>,
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        active_cell: Option<crate::GridCellAddress>,
        transpose: bool,
        command: &mut Option<EditorCommand>,
    ) {
        let Some(anchor) = active_cell else {
            editor.state.status = Some("Select a paste anchor cell first.".to_owned());
            return;
        };
        match clipboard
            .as_mut()
            .and_then(|clipboard| clipboard.get_text().ok())
        {
            Some(text) => {
                *command = Some(EditorCommand::PasteGridCells {
                    grid,
                    anchor,
                    clipboard: crate::ClipboardTable::parse_tsv_auto(&text),
                    transpose,
                });
            }
            None => editor.state.status = Some("Clipboard text is unavailable.".to_owned()),
        }
    }

    fn handle_table_keyboard(
        ui: &egui::Ui,
        editor: &mut TetraplotEditor,
        grid: crate::CompositionGridId,
        clipboard: &mut Option<arboard::Clipboard>,
        selected: Option<&crate::ClipboardTable>,
        command: &mut Option<EditorCommand>,
    ) {
        let (up, down, left, right, tab, enter, select_all, copy, paste, clear, shift) =
            ui.input(|input| {
                (
                    input.key_pressed(egui::Key::ArrowUp),
                    input.key_pressed(egui::Key::ArrowDown),
                    input.key_pressed(egui::Key::ArrowLeft),
                    input.key_pressed(egui::Key::ArrowRight),
                    input.key_pressed(egui::Key::Tab),
                    input.key_pressed(egui::Key::Enter),
                    input.modifiers.command && input.key_pressed(egui::Key::A),
                    input.modifiers.command && input.key_pressed(egui::Key::C),
                    input.modifiers.command && input.key_pressed(egui::Key::V),
                    input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace),
                    input.modifiers.shift,
                )
            });
        let Some(table) = editor.state.tables.get_mut(&grid) else {
            return;
        };
        if select_all {
            table.select_all();
        }
        let moved = if up || (enter && shift) {
            table.move_active(-1, 0, shift)
        } else if down || enter {
            table.move_active(1, 0, shift)
        } else if left || (tab && shift) {
            table.move_active(0, -1, shift)
        } else if right || tab {
            table.move_active(0, 1, shift)
        } else {
            false
        };
        let active = table.active_cell;
        let selected_cells = table.selected_cells();
        if moved && let Some(active) = active {
            *command = Some(EditorCommand::SetSelection(Selection::GridRow {
                grid,
                row: active.row,
            }));
        }
        if copy {
            copy_table_to_clipboard(clipboard, selected, false, &mut editor.state.status);
        }
        if paste {
            paste_from_clipboard(clipboard, editor, grid, active, false, command);
        }
        if clear && !selected_cells.is_empty() {
            *command = Some(EditorCommand::ClearGridCells {
                grid,
                cells: selected_cells,
            });
        }
    }

    fn event_in_viewport(event: &three_d::Event, viewport: crate::EditorViewport) -> bool {
        let (position, handled) = match event {
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
            } => (*position, *handled),
            _ => return false,
        };
        !handled && viewport.contains_physical([position.x, position.y])
    }

    fn handle_3d_pick(
        events: &[three_d::Event],
        camera: &NativeCamera,
        editor: &mut TetraplotEditor,
        _viewport: crate::EditorViewport,
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
        let grid_points: Vec<_> = editor
            .document
            .grids()
            .filter_map(|grid| grid.id())
            .flat_map(|grid| {
                editor
                    .document
                    .prepared_grid_points(grid)
                    .unwrap_or_default()
            })
            .collect();
        if let Some(hit) = crate::pick_grid_points(ray, &grid_points, 0.035)
            && let (Some(grid), Some(row)) = (hit.grid, hit.grid_row)
        {
            if let Err(error) = editor.state.select_grid_row(&editor.document, grid, row) {
                editor.state.status = Some(error.to_string());
            } else {
                editor.state.status = Some(format!(
                    "Selected grid {} row {} at [{:.4}, {:.4}, {:.4}]",
                    grid.get(),
                    row.get(),
                    hit.world_position[0],
                    hit.world_position[1],
                    hit.world_position[2]
                ));
                if let Some(local) = hit.local_position {
                    let target = match editor
                        .document
                        .grid(grid)
                        .map(|grid| grid.coordinate_space())
                    {
                        Some(crate::GridCoordinateSpace::Section(section)) => {
                            Some(FlatViewTarget::Section(section))
                        }
                        Some(crate::GridCoordinateSpace::EmbeddedChart(chart)) => {
                            Some(FlatViewTarget::EmbeddedChart(chart))
                        }
                        _ => None,
                    };
                    if let Some(target) = target {
                        set_linked_cursor(editor, target, local);
                    }
                }
            }
            return;
        }
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
