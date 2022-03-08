#![forbid(unsafe_code)]
#![allow(irrefutable_let_patterns)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::large_enum_variant)]
#![allow(clippy::eval_order_dependence)]
// These are useless.
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::inconsistent_struct_constructor)]

extern crate fyrox;
#[macro_use]
extern crate lazy_static;
extern crate directories;

mod asset;
mod audio;
mod camera;
mod command;
mod configurator;
mod curve_editor;
mod gui;
mod inspector;
mod interaction;
mod light;
mod log;
mod material;
mod menu;
mod overlay;
mod preview;
mod project_dirs;
mod scene;
mod scene_viewer;
mod settings;
mod utils;
mod world;

use crate::utils::normalize_os_event;
use crate::{
    asset::{item::AssetItem, item::AssetKind, AssetBrowser},
    audio::AudioPanel,
    command::{panel::CommandStackViewer, Command, CommandStack},
    configurator::Configurator,
    curve_editor::CurveEditorWindow,
    inspector::Inspector,
    interaction::{
        move_mode::MoveInteractionMode,
        navmesh::{EditNavmeshMode, NavmeshPanel},
        rotate_mode::RotateInteractionMode,
        scale_mode::ScaleInteractionMode,
        select_mode::SelectInteractionMode,
        terrain::TerrainInteractionMode,
        InteractionMode, InteractionModeKind,
    },
    light::LightPanel,
    log::Log,
    material::MaterialEditor,
    menu::{Menu, MenuContext, Panels},
    overlay::OverlayRenderPass,
    scene::{
        commands::{
            graph::AddModelCommand, make_delete_selection_command, mesh::SetMeshTextureCommand,
            particle_system::SetParticleSystemTextureCommand, sprite::SetSpriteTextureCommand,
            ChangeSelectionCommand, CommandGroup, PasteCommand, SceneCommand, SceneContext,
        },
        EditorScene, Selection,
    },
    scene_viewer::SceneViewer,
    settings::{Settings, SettingsSectionKind},
    utils::path_fixer::PathFixer,
    world::{graph::selection::GraphSelection, WorldViewer},
};
use fyrox::plugin::Plugin;
use fyrox::{
    core::{
        algebra::Vector2,
        color::Color,
        futures::executor::block_on,
        parking_lot::Mutex,
        pool::{ErasedHandle, Handle},
        scope_profile,
        sstorage::ImmutableString,
    },
    dpi::LogicalSize,
    engine::{resource_manager::ResourceManager, Engine, EngineInitParams, SerializationContext},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::{
        brush::Brush,
        dock::{DockingManagerBuilder, TileBuilder, TileContent},
        draw,
        dropdown_list::DropdownListBuilder,
        file_browser::{FileBrowserMode, FileSelectorBuilder, FileSelectorMessage, Filter},
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        messagebox::{MessageBoxBuilder, MessageBoxButtons, MessageBoxMessage, MessageBoxResult},
        ttf::Font,
        widget::{WidgetBuilder, WidgetMessage},
        window::{WindowBuilder, WindowMessage, WindowTitle},
        BuildContext, UiNode, UserInterface, VerticalAlignment,
    },
    material::{shader::Shader, Material, PropertyValue},
    resource::texture::{CompressionOptions, Texture, TextureKind},
    scene::{
        camera::{Camera, Projection},
        mesh::Mesh,
        node::Node,
        Scene, SceneLoader,
    },
    utils::{
        into_gui_texture, log::MessageKind, translate_cursor_icon, translate_event,
        watcher::FileSystemWatcher,
    },
};
use std::{
    any::TypeId,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::from_utf8,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

pub const FIXED_TIMESTEP: f32 = 1.0 / 60.0;
pub const MSG_SYNC_FLAG: u64 = 1;

pub fn send_sync_message(ui: &UserInterface, mut msg: UiMessage) {
    msg.flags = MSG_SYNC_FLAG;
    ui.send_message(msg);
}

type GameEngine = fyrox::engine::Engine;

lazy_static! {
    // This checks release.toml debug handle and at
    // the same time checks if program is installed
    static ref DEBUG_HANDLE: bool = {
        let release_toml = project_dirs::resources_dir("release.toml");
        if release_toml.exists() {
            let file = fs::read(release_toml).unwrap();
            from_utf8(&file)
                .unwrap()
                .parse::<toml::Value>()
                .unwrap()["debug-mode"]
                .as_bool()
                .unwrap()
        } else {
            true
        }
    };

    // This constant gives DEBUG_HANDLE value to config_dir and data_dir
    // functions and checks if config and data dir are created.
    static ref TEST_EXISTENCE: bool = {
        if !(*DEBUG_HANDLE) {
            // We check if config and data dir exists
            if !project_dirs::data_dir("").exists() {
                // If there's aren't any, we create them.
                fs::create_dir(project_dirs::config_dir("")).unwrap();
                fs::create_dir(project_dirs::data_dir("")).unwrap();
            }

            true
        } else {
            false
        }
    };

    static ref CONFIG_DIR: Mutex<PathBuf> = Mutex::new(project_dirs::working_config_dir(""));
    static ref DATA_DIR: Mutex<PathBuf> = Mutex::new(project_dirs::working_data_dir(""));
}

pub fn load_image(data: &[u8]) -> Option<draw::SharedTexture> {
    Some(into_gui_texture(
        Texture::load_from_memory(data, CompressionOptions::NoCompression, false).ok()?,
    ))
}

lazy_static! {
    static ref GIZMO_SHADER: Shader = {
        Shader::from_str(
            include_str!("../resources/embed/shaders/gizmo.shader",),
            PathBuf::default(),
        )
        .unwrap()
    };
}

pub fn make_color_material(color: Color) -> Arc<Mutex<Material>> {
    let mut material = Material::from_shader(GIZMO_SHADER.clone(), None);
    material
        .set_property(
            &ImmutableString::new("diffuseColor"),
            PropertyValue::Color(color),
        )
        .unwrap();
    Arc::new(Mutex::new(material))
}

pub fn set_mesh_diffuse_color(mesh: &mut Mesh, color: Color) {
    for surface in mesh.surfaces() {
        surface
            .material()
            .lock()
            .set_property(
                &ImmutableString::new("diffuseColor"),
                PropertyValue::Color(color),
            )
            .unwrap();
    }
}

pub fn create_terrain_layer_material() -> Arc<Mutex<Material>> {
    let mut material = Material::standard_terrain();
    material
        .set_property(
            &ImmutableString::new("texCoordScale"),
            PropertyValue::Vector2(Vector2::new(10.0, 10.0)),
        )
        .unwrap();
    Arc::new(Mutex::new(material))
}

#[derive(Debug)]
pub enum Message {
    DoSceneCommand(SceneCommand),
    UndoSceneCommand,
    RedoSceneCommand,
    ClearSceneCommandStack,
    SelectionChanged,
    SyncToModel,
    SaveScene(PathBuf),
    LoadScene(PathBuf),
    CloseScene,
    SetInteractionMode(InteractionModeKind),
    Log(String),
    Configure {
        working_directory: PathBuf,
    },
    NewScene,
    Exit {
        force: bool,
    },
    OpenSettings(SettingsSectionKind),
    OpenMaterialEditor(Arc<Mutex<Material>>),
    ShowInAssetBrowser(PathBuf),
    SetWorldViewerFilter(String),
    LocateObject {
        type_id: TypeId,
        handle: ErasedHandle,
    },
    SelectObject {
        type_id: TypeId,
        handle: ErasedHandle,
    },
    SetEditorCameraProjection(Projection),
    SwitchToPlayMode,
    SwitchToEditMode,
    SwitchMode,
    OpenLoadSceneDialog,
}

impl Message {
    pub fn do_scene_command<C: Command>(cmd: C) -> Self {
        Self::DoSceneCommand(SceneCommand::new(cmd))
    }
}

pub fn make_scene_file_filter() -> Filter {
    Filter::new(|p: &Path| {
        if let Some(ext) = p.extension() {
            ext.to_string_lossy().as_ref() == "rgs"
        } else {
            p.is_dir()
        }
    })
}

pub fn make_save_file_selector(ctx: &mut BuildContext) -> Handle<UiNode> {
    FileSelectorBuilder::new(
        WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(400.0))
            .with_title(WindowTitle::Text("Save Scene As".into()))
            .open(false),
    )
    .with_mode(FileBrowserMode::Save {
        default_file_name: PathBuf::from("unnamed.rgs"),
    })
    .with_path("./")
    .with_filter(make_scene_file_filter())
    .build(ctx)
}

pub enum Mode {
    Edit,
    Play {
        // Play mode scene.
        scene: Handle<Scene>,
        // List of scenes that existed before entering play mode.
        existing_scenes: Vec<Handle<Scene>>,
    },
}

impl Mode {
    pub fn is_edit(&self) -> bool {
        !self.is_play()
    }

    pub fn is_play(&self) -> bool {
        matches!(self, Mode::Play { .. })
    }
}

pub struct GameLoopData {
    clock: Instant,
    elapsed_time: f32,
}

pub struct StartupData {
    /// Working directory that should be set when starting the editor. If it is empty, then
    /// current working directory won't be changed.
    pub working_directory: PathBuf,

    /// A scene to load at the editor start. If it is empty, no scene will be loaded.
    pub scene: PathBuf,
}

pub struct Editor {
    game_loop_data: GameLoopData,
    engine: Engine,
    scene: Option<EditorScene>,
    command_stack: CommandStack,
    message_sender: Sender<Message>,
    message_receiver: Receiver<Message>,
    interaction_modes: Vec<Box<dyn InteractionMode>>,
    current_interaction_mode: Option<InteractionModeKind>,
    world_viewer: WorldViewer,
    root_grid: Handle<UiNode>,
    scene_viewer: SceneViewer,
    asset_browser: AssetBrowser,
    exit_message_box: Handle<UiNode>,
    save_file_selector: Handle<UiNode>,
    light_panel: LightPanel,
    menu: Menu,
    exit: bool,
    configurator: Configurator,
    log: Log,
    command_stack_viewer: CommandStackViewer,
    validation_message_box: Handle<UiNode>,
    navmesh_panel: NavmeshPanel,
    settings: Settings,
    path_fixer: PathFixer,
    material_editor: MaterialEditor,
    inspector: Inspector,
    curve_editor: CurveEditorWindow,
    audio_panel: AudioPanel,
    mode: Mode,
}

impl Editor {
    pub fn new(event_loop: &EventLoop<()>, startup_data: Option<StartupData>) -> Self {
        let inner_size = if let Some(primary_monitor) = event_loop.primary_monitor() {
            let mut monitor_dimensions = primary_monitor.size();
            monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
            monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
            monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor())
        } else {
            LogicalSize::new(1024.0, 768.0)
        };

        let window_builder = fyrox::window::WindowBuilder::new()
            .with_inner_size(inner_size)
            .with_title("Fyroxed")
            .with_resizable(true);

        let serialization_context = Arc::new(SerializationContext::new());
        let mut engine = Engine::new(EngineInitParams {
            window_builder,
            resource_manager: ResourceManager::new(serialization_context.clone()),
            serialization_context,
            events_loop: event_loop,
            vsync: true,
        })
        .unwrap();

        let overlay_pass = OverlayRenderPass::new(engine.renderer.pipeline_state());
        engine.renderer.add_render_pass(overlay_pass);

        let (message_sender, message_receiver) = mpsc::channel();

        *fyrox::gui::DEFAULT_FONT.0.lock().unwrap() = Font::from_memory(
            include_bytes!("../resources/embed/arial.ttf").to_vec(),
            14.0,
            Font::default_char_set(),
        )
        .unwrap();

        let configurator = Configurator::new(
            message_sender.clone(),
            &mut engine.user_interface.build_ctx(),
        );

        let mut settings = Settings::default();

        match Settings::load() {
            Ok(s) => {
                settings = s;

                println!("Editor settings were loaded successfully!");

                match engine
                    .renderer
                    .set_quality_settings(&settings.graphics.quality)
                {
                    Ok(_) => {
                        println!("Graphics settings were applied successfully!");
                    }
                    Err(e) => {
                        println!("Failed to apply graphics settings! Reason: {:?}", e)
                    }
                }
            }
            Err(e) => {
                println!(
                    "Failed to load settings, fallback to default. Reason: {:?}",
                    e
                )
            }
        }

        let scene_viewer = SceneViewer::new(&mut engine, message_sender.clone());
        let asset_browser = AssetBrowser::new(&mut engine);
        let menu = Menu::new(&mut engine, message_sender.clone(), &settings);
        let light_panel = LightPanel::new(&mut engine);
        let audio_panel = AudioPanel::new(&mut engine);

        let ctx = &mut engine.user_interface.build_ctx();
        let navmesh_panel = NavmeshPanel::new(ctx, message_sender.clone());
        let world_outliner = WorldViewer::new(ctx, message_sender.clone());
        let command_stack_viewer = CommandStackViewer::new(ctx, message_sender.clone());
        let log = Log::new(ctx);
        let inspector = Inspector::new(ctx, message_sender.clone());

        let root_grid = GridBuilder::new(
            WidgetBuilder::new()
                .with_width(engine.renderer.get_frame_size().0 as f32)
                .with_height(engine.renderer.get_frame_size().1 as f32)
                .with_child(menu.menu)
                .with_child(
                    DockingManagerBuilder::new(WidgetBuilder::new().on_row(1).with_child({
                        TileBuilder::new(WidgetBuilder::new())
                            .with_content(TileContent::VerticalTiles {
                                splitter: 0.75,
                                tiles: [
                                    TileBuilder::new(WidgetBuilder::new())
                                        .with_content(TileContent::HorizontalTiles {
                                            splitter: 0.25,
                                            tiles: [
                                                TileBuilder::new(WidgetBuilder::new())
                                                    .with_content(TileContent::Window(
                                                        world_outliner.window,
                                                    ))
                                                    .build(ctx),
                                                TileBuilder::new(WidgetBuilder::new())
                                                    .with_content(TileContent::HorizontalTiles {
                                                        splitter: 0.66,
                                                        tiles: [
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(TileContent::Window(
                                                                    scene_viewer.window(),
                                                                ))
                                                                .build(ctx),
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(TileContent::Window(
                                                                    inspector.window,
                                                                ))
                                                                .build(ctx),
                                                        ],
                                                    })
                                                    .build(ctx),
                                            ],
                                        })
                                        .build(ctx),
                                    TileBuilder::new(WidgetBuilder::new())
                                        .with_content(TileContent::HorizontalTiles {
                                            splitter: 0.66,
                                            tiles: [
                                                TileBuilder::new(WidgetBuilder::new())
                                                    .with_content(TileContent::HorizontalTiles {
                                                        splitter: 0.80,
                                                        tiles: [
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(TileContent::Window(
                                                                    asset_browser.window,
                                                                ))
                                                                .build(ctx),
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(TileContent::Window(
                                                                    command_stack_viewer.window,
                                                                ))
                                                                .build(ctx),
                                                        ],
                                                    })
                                                    .build(ctx),
                                                TileBuilder::new(WidgetBuilder::new())
                                                    .with_content(TileContent::HorizontalTiles {
                                                        splitter: 0.5,
                                                        tiles: [
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(TileContent::Window(
                                                                    log.window,
                                                                ))
                                                                .build(ctx),
                                                            TileBuilder::new(WidgetBuilder::new())
                                                                .with_content(
                                                                    TileContent::HorizontalTiles {
                                                                        splitter: 0.5,
                                                                        tiles: [
                                                                            TileBuilder::new(
                                                                                WidgetBuilder::new(
                                                                                ),
                                                                            )
                                                                            .with_content(
                                                                                TileContent::Window(
                                                                                    navmesh_panel
                                                                                        .window,
                                                                                ),
                                                                            )
                                                                            .build(ctx),
                                                                            audio_panel.window,
                                                                        ],
                                                                    },
                                                                )
                                                                .build(ctx),
                                                        ],
                                                    })
                                                    .build(ctx),
                                            ],
                                        })
                                        .build(ctx),
                                ],
                            })
                            .build(ctx)
                    }))
                    .build(ctx),
                ),
        )
        .add_row(Row::strict(25.0))
        .add_row(Row::stretch())
        .add_column(Column::stretch())
        .build(ctx);

        let save_file_selector = make_save_file_selector(ctx);

        let exit_message_box = MessageBoxBuilder::new(
            WindowBuilder::new(WidgetBuilder::new().with_width(300.0).with_height(100.0))
                .can_close(false)
                .can_minimize(false)
                .open(false)
                .with_title(WindowTitle::Text("Unsaved changes".to_owned())),
        )
        .with_text("There are unsaved changes. Do you wish to save them before exit?")
        .with_buttons(MessageBoxButtons::YesNoCancel)
        .build(ctx);

        let validation_message_box = MessageBoxBuilder::new(
            WindowBuilder::new(WidgetBuilder::new().with_width(400.0).with_height(500.0))
                .can_close(false)
                .can_minimize(false)
                .open(false)
                .with_title(WindowTitle::Text("Validation failed!".to_owned())),
        )
        .with_buttons(MessageBoxButtons::Ok)
        .build(ctx);

        let path_fixer = PathFixer::new(ctx);

        let curve_editor = CurveEditorWindow::new(ctx);

        let material_editor = MaterialEditor::new(&mut engine);

        let mut editor = Self {
            engine,
            navmesh_panel,
            scene_viewer,
            scene: None,
            command_stack: CommandStack::new(false),
            message_sender,
            message_receiver,
            interaction_modes: Default::default(),
            current_interaction_mode: None,
            world_viewer: world_outliner,
            root_grid,
            menu,
            exit: false,
            asset_browser,
            exit_message_box,
            save_file_selector,
            configurator,
            log,
            light_panel,
            command_stack_viewer,
            validation_message_box,
            settings,
            path_fixer,
            material_editor,
            inspector,
            curve_editor,
            audio_panel,
            mode: Mode::Edit,
            game_loop_data: GameLoopData {
                clock: Instant::now(),
                elapsed_time: 0.0,
            },
        };

        editor.set_interaction_mode(Some(InteractionModeKind::Move));

        if let Some(data) = startup_data {
            editor
                .message_sender
                .send(Message::Configure {
                    working_directory: if data.working_directory == PathBuf::default() {
                        std::env::current_dir().unwrap()
                    } else {
                        data.working_directory
                    },
                })
                .unwrap();

            if data.scene != PathBuf::default() {
                editor
                    .message_sender
                    .send(Message::LoadScene(data.scene))
                    .unwrap();
            }
        } else {
            // Open configurator as usual.
            editor
                .engine
                .user_interface
                .send_message(WindowMessage::open_modal(
                    editor.configurator.window,
                    MessageDirection::ToWidget,
                    true,
                ));
        }

        editor
    }

    fn set_scene(&mut self, mut scene: Scene, path: Option<PathBuf>) {
        if let Some(previous_editor_scene) = self.scene.as_ref() {
            self.engine.scenes.remove(previous_editor_scene.scene);
        }
        self.scene = None;
        self.sync_to_model();
        poll_ui_messages(self);

        scene.render_target = Some(Texture::new_render_target(0, 0));
        self.scene_viewer
            .set_render_target(&self.engine.user_interface, scene.render_target.clone());

        let editor_scene = EditorScene::from_native_scene(scene, &mut self.engine, path.clone());

        for mut interaction_mode in self.interaction_modes.drain(..) {
            interaction_mode.on_drop(&mut self.engine);
        }

        self.interaction_modes = vec![
            Box::new(SelectInteractionMode::new(
                self.scene_viewer.frame(),
                self.scene_viewer.selection_frame(),
                self.message_sender.clone(),
            )),
            Box::new(MoveInteractionMode::new(
                &editor_scene,
                &mut self.engine,
                self.message_sender.clone(),
            )),
            Box::new(ScaleInteractionMode::new(
                &editor_scene,
                &mut self.engine,
                self.message_sender.clone(),
            )),
            Box::new(RotateInteractionMode::new(
                &editor_scene,
                &mut self.engine,
                self.message_sender.clone(),
            )),
            Box::new(EditNavmeshMode::new(
                &editor_scene,
                &mut self.engine,
                self.message_sender.clone(),
            )),
            Box::new(TerrainInteractionMode::new(
                &editor_scene,
                &mut self.engine,
                self.message_sender.clone(),
            )),
        ];

        self.command_stack = CommandStack::new(false);
        self.scene = Some(editor_scene);

        self.set_interaction_mode(Some(InteractionModeKind::Move));
        self.sync_to_model();

        self.scene_viewer.set_title(
            &self.engine.user_interface,
            format!(
                "Scene Preview - {}",
                path.map_or("Unnamed Scene".to_string(), |p| p
                    .to_string_lossy()
                    .to_string())
            ),
        );
        self.engine.renderer.flush();
    }

    fn set_interaction_mode(&mut self, mode: Option<InteractionModeKind>) {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_ref() {
            if self.current_interaction_mode != mode {
                // Deactivate current first.
                if let Some(current_mode) = self.current_interaction_mode {
                    self.interaction_modes[current_mode as usize].deactivate(editor_scene, engine);
                }

                self.current_interaction_mode = mode;

                // Activate new.
                if let Some(current_mode) = self.current_interaction_mode {
                    self.interaction_modes[current_mode as usize].activate(editor_scene, engine);
                }
            }
        }
    }

    pub fn handle_ui_message(&mut self, message: &UiMessage) {
        scope_profile!();

        // Prevent infinite message loops.
        if message.has_flags(MSG_SYNC_FLAG) {
            return;
        }

        let engine = &mut self.engine;

        self.configurator.handle_ui_message(message, engine);
        self.menu.handle_ui_message(
            message,
            MenuContext {
                engine,
                editor_scene: self.scene.as_mut(),
                panels: Panels {
                    inspector_window: self.inspector.window,
                    world_outliner_window: self.world_viewer.window,
                    asset_window: self.asset_browser.window,
                    light_panel: self.light_panel.window,
                    log_panel: self.log.window,
                    configurator_window: self.configurator.window,
                    path_fixer: self.path_fixer.window,
                    curve_editor: &self.curve_editor,
                },
                settings: &mut self.settings,
            },
        );

        self.log.handle_ui_message(message, engine);
        self.asset_browser
            .handle_ui_message(message, engine, self.message_sender.clone());
        self.command_stack_viewer.handle_ui_message(message);
        self.curve_editor.handle_ui_message(message, engine);
        self.path_fixer.handle_ui_message(
            message,
            &mut engine.user_interface,
            engine.serialization_context.clone(),
            engine.resource_manager.clone(),
        );
        self.scene_viewer.handle_ui_message(
            message,
            engine,
            self.scene.as_mut(),
            self.current_interaction_mode
                .and_then(|i| self.interaction_modes.get_mut(i as usize)),
            &self.settings,
            &self.mode,
        );

        if let Some(editor_scene) = self.scene.as_mut() {
            self.audio_panel
                .handle_ui_message(message, editor_scene, &self.message_sender, engine);

            self.navmesh_panel.handle_message(
                message,
                editor_scene,
                engine,
                if let Some(edit_mode) = self.interaction_modes
                    [InteractionModeKind::Navmesh as usize]
                    .as_any_mut()
                    .downcast_mut()
                {
                    edit_mode
                } else {
                    unreachable!()
                },
            );

            self.inspector
                .handle_ui_message(message, editor_scene, engine, &self.message_sender);

            if let Some(current_im) = self.current_interaction_mode {
                self.interaction_modes[current_im as usize].handle_ui_message(
                    message,
                    editor_scene,
                    engine,
                );
            }

            self.world_viewer
                .handle_ui_message(message, editor_scene, engine);

            self.light_panel
                .handle_ui_message(message, editor_scene, engine);

            self.material_editor
                .handle_ui_message(message, engine, &self.message_sender);

            if let Some(MessageBoxMessage::Close(result)) = message.data::<MessageBoxMessage>() {
                if message.destination() == self.exit_message_box {
                    match result {
                        MessageBoxResult::No => {
                            self.message_sender
                                .send(Message::Exit { force: true })
                                .unwrap();
                        }
                        MessageBoxResult::Yes => {
                            if let Some(scene) = self.scene.as_ref() {
                                if let Some(path) = scene.path.as_ref() {
                                    self.message_sender
                                        .send(Message::SaveScene(path.clone()))
                                        .unwrap();
                                    self.message_sender
                                        .send(Message::Exit { force: true })
                                        .unwrap();
                                } else {
                                    // Scene wasn't saved yet, open Save As dialog.
                                    engine
                                        .user_interface
                                        .send_message(WindowMessage::open_modal(
                                            self.save_file_selector,
                                            MessageDirection::ToWidget,
                                            true,
                                        ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            } else if let Some(FileSelectorMessage::Commit(path)) =
                message.data::<FileSelectorMessage>()
            {
                if message.destination() == self.save_file_selector {
                    self.message_sender
                        .send(Message::SaveScene(path.clone()))
                        .unwrap();
                    self.message_sender
                        .send(Message::Exit { force: true })
                        .unwrap();
                }
            }
        }
    }

    fn set_play_mode(&mut self) {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_ref() {
            let mut purified_scene = editor_scene.make_purified_scene(engine);

            // Hack. Turn on cameras.
            for node in purified_scene.graph.linear_iter_mut() {
                if let Some(camera) = node.cast_mut::<Camera>() {
                    camera.set_enabled(true);
                }
            }

            purified_scene.drawing_context.clear_lines();
            purified_scene.render_target = Some(Texture::new_render_target(0, 0));

            // Force previewer to use play-mode scene.
            self.scene_viewer
                .set_render_target(&engine.user_interface, purified_scene.render_target.clone());

            let existing_scenes = engine
                .scenes
                .pair_iter()
                .map(|(h, _)| h)
                .collect::<Vec<_>>();

            let handle = engine.scenes.add(purified_scene);

            engine.call_plugins_on_enter_play_mode(handle, FIXED_TIMESTEP, true);

            // Initialize scripts.
            engine.initialize_scene_scripts(handle, 0.0);

            self.mode = Mode::Play {
                scene: handle,
                existing_scenes,
            };
            self.on_mode_changed();
        }
    }

    fn set_editor_mode(&mut self) {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_ref() {
            // Destroy play mode scene.
            if let Mode::Play {
                scene,
                existing_scenes,
            } = std::mem::replace(&mut self.mode, Mode::Edit)
            {
                engine.call_plugins_on_leave_play_mode(FIXED_TIMESTEP, true);

                // Remove play mode scene.
                engine.scenes.remove(scene);

                // Remove every scene that was created in the play mode.
                let scenes_to_destroy = engine
                    .scenes
                    .pair_iter()
                    .filter_map(|(h, _)| {
                        if existing_scenes.contains(&h) {
                            // Keep existing scenes.
                            None
                        } else {
                            // Destroy scenes that were created in play mode.
                            Some(h)
                        }
                    })
                    .collect::<Vec<_>>();

                for scene_to_destroy in scenes_to_destroy {
                    engine.scenes.remove(scene_to_destroy);
                }

                // Force previewer to use editor's scene.
                let render_target = engine.scenes[editor_scene.scene].render_target.clone();
                self.scene_viewer
                    .set_render_target(&engine.user_interface, render_target);

                self.on_mode_changed();
            }
        }
    }

    fn on_mode_changed(&mut self) {
        let engine = &mut self.engine;
        let ui = &engine.user_interface;
        self.scene_viewer.on_mode_changed(ui, &self.mode);
        self.world_viewer.on_mode_changed(ui, &self.mode);
        self.asset_browser.on_mode_changed(ui, &self.mode);
        self.command_stack_viewer.on_mode_changed(ui, &self.mode);
        self.inspector.on_mode_changed(ui, &self.mode);
        self.audio_panel.on_mode_changed(ui, &self.mode);
        self.navmesh_panel.on_mode_changed(ui, &self.mode);
        self.menu.on_mode_changed(ui, &self.mode);
    }

    fn sync_to_model(&mut self) {
        scope_profile!();

        let engine = &mut self.engine;

        self.menu
            .sync_to_model(self.scene.as_ref(), &mut engine.user_interface);

        if let Some(editor_scene) = self.scene.as_mut() {
            self.inspector.sync_to_model(editor_scene, engine);
            self.navmesh_panel.sync_to_model(editor_scene, engine);
            self.world_viewer.sync_to_model(editor_scene, engine);
            self.material_editor
                .sync_to_model(&mut engine.user_interface);
            self.audio_panel.sync_to_model(editor_scene, engine);
            self.command_stack_viewer.sync_to_model(
                &mut self.command_stack,
                &SceneContext {
                    scene: &mut engine.scenes[editor_scene.scene],
                    message_sender: self.message_sender.clone(),
                    editor_scene,
                    resource_manager: engine.resource_manager.clone(),
                    serialization_context: engine.serialization_context.clone(),
                },
                &mut engine.user_interface,
            )
        } else {
            self.inspector.clear(&engine.user_interface);
            self.world_viewer.clear(&engine.user_interface);
        }
    }

    fn post_update(&mut self) {
        if let Some(scene) = self.scene.as_mut() {
            self.world_viewer.post_update(scene, &mut self.engine);
        }
    }

    fn handle_resize(&mut self) {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_ref() {
            let scene = match self.mode {
                Mode::Edit => &mut engine.scenes[editor_scene.scene],
                Mode::Play { scene, .. } => &mut engine.scenes[scene],
            };

            // Create new render target if preview frame has changed its size.
            if let TextureKind::Rectangle { width, height } =
                scene.render_target.clone().unwrap().data_ref().kind()
            {
                let frame_size = self.scene_viewer.frame_bounds(&engine.user_interface).size;
                if width != frame_size.x as u32 || height != frame_size.y as u32 {
                    scene.render_target = Some(Texture::new_render_target(
                        frame_size.x as u32,
                        frame_size.y as u32,
                    ));
                    self.scene_viewer
                        .set_render_target(&engine.user_interface, scene.render_target.clone());
                }
            }
        }
    }

    fn do_scene_command(&mut self, command: SceneCommand) -> bool {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_mut() {
            self.command_stack.do_command(
                command.into_inner(),
                SceneContext {
                    scene: &mut engine.scenes[editor_scene.scene],
                    message_sender: self.message_sender.clone(),
                    editor_scene,
                    resource_manager: engine.resource_manager.clone(),
                    serialization_context: engine.serialization_context.clone(),
                },
            );
            true
        } else {
            false
        }
    }

    fn undo_scene_command(&mut self) -> bool {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_mut() {
            self.command_stack.undo(SceneContext {
                scene: &mut engine.scenes[editor_scene.scene],
                message_sender: self.message_sender.clone(),
                editor_scene,
                resource_manager: engine.resource_manager.clone(),
                serialization_context: engine.serialization_context.clone(),
            });
            true
        } else {
            false
        }
    }

    fn redo_scene_command(&mut self) -> bool {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_mut() {
            self.command_stack.redo(SceneContext {
                scene: &mut engine.scenes[editor_scene.scene],
                message_sender: self.message_sender.clone(),
                editor_scene,
                resource_manager: engine.resource_manager.clone(),
                serialization_context: engine.serialization_context.clone(),
            });
            true
        } else {
            false
        }
    }

    fn clear_scene_command_stack(&mut self) -> bool {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_mut() {
            self.command_stack.clear(SceneContext {
                scene: &mut engine.scenes[editor_scene.scene],
                message_sender: self.message_sender.clone(),
                editor_scene,
                resource_manager: engine.resource_manager.clone(),
                serialization_context: engine.serialization_context.clone(),
            });
            true
        } else {
            false
        }
    }

    fn save_current_scene(&mut self, path: PathBuf) {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.as_mut() {
            match editor_scene.save(path.clone(), engine) {
                Ok(message) => {
                    self.scene_viewer.set_title(
                        &engine.user_interface,
                        format!("Scene Preview - {}", path.display()),
                    );

                    self.message_sender.send(Message::Log(message)).unwrap();
                }
                Err(message) => {
                    self.message_sender
                        .send(Message::Log(message.clone()))
                        .unwrap();

                    engine.user_interface.send_message(MessageBoxMessage::open(
                        self.validation_message_box,
                        MessageDirection::ToWidget,
                        None,
                        Some(message),
                    ));
                }
            }
        }
    }

    fn load_scene(&mut self, scene_path: PathBuf) {
        let engine = &mut self.engine;
        let result = {
            block_on(SceneLoader::from_file(
                &scene_path,
                engine.serialization_context.clone(),
            ))
        };
        match result {
            Ok(loader) => {
                let scene = block_on(loader.finish(engine.resource_manager.clone()));

                self.set_scene(scene, Some(scene_path));
            }
            Err(e) => {
                self.message_sender
                    .send(Message::Log(e.to_string()))
                    .unwrap();
            }
        }
    }

    fn exit(&mut self, force: bool) {
        let engine = &mut self.engine;
        if force {
            self.exit = true;
        } else if self.scene.is_some() {
            engine.user_interface.send_message(MessageBoxMessage::open(
                self.exit_message_box,
                MessageDirection::ToWidget,
                None,
                None,
            ));
        } else {
            self.exit = true;
        }
    }

    fn close_current_scene(&mut self) -> bool {
        let engine = &mut self.engine;
        if let Some(editor_scene) = self.scene.take() {
            engine.scenes.remove(editor_scene.scene);

            // Preview frame has scene frame texture assigned, it must be cleared explicitly,
            // otherwise it will show last rendered frame in preview which is not what we want.
            self.scene_viewer
                .set_render_target(&engine.user_interface, None);
            // Set default title scene
            self.scene_viewer
                .set_title(&engine.user_interface, "Scene Preview".to_string());

            true
        } else {
            false
        }
    }

    fn create_new_scene(&mut self) {
        let mut scene = Scene::new();

        scene.ambient_lighting_color = Color::opaque(200, 200, 200);

        self.set_scene(scene, None);
    }

    fn configure(&mut self, working_directory: PathBuf) {
        let engine = &mut self.engine;

        assert!(self.scene.is_none());

        self.asset_browser.clear_preview(engine);

        std::env::set_current_dir(working_directory.clone()).unwrap();

        engine
            .get_window()
            .set_title(&format!("Fyroxed: {}", working_directory.to_string_lossy()));

        match FileSystemWatcher::new(&working_directory, Duration::from_secs(1)) {
            Ok(watcher) => {
                engine.resource_manager.state().set_watcher(Some(watcher));
            }
            Err(e) => {
                self.message_sender
                    .send(Message::Log(format!(
                        "Unable to create resource watcher. Reason {:?}",
                        e
                    )))
                    .unwrap();
            }
        }

        engine.resource_manager.state().destroy_unused_resources();

        engine.renderer.flush();

        self.asset_browser
            .set_working_directory(engine, &working_directory);

        self.message_sender
            .send(Message::Log(format!(
                "New working directory was successfully set: {:?}",
                working_directory
            )))
            .unwrap();
    }

    fn select_object(&mut self, type_id: TypeId, handle: ErasedHandle) {
        if let Some(scene) = self.scene.as_ref() {
            let new_selection = if type_id == TypeId::of::<Node>() {
                Some(Selection::Graph(GraphSelection::single_or_empty(
                    handle.into(),
                )))
            } else {
                None
            };

            if let Some(new_selection) = new_selection {
                self.message_sender
                    .send(Message::DoSceneCommand(SceneCommand::new(
                        ChangeSelectionCommand::new(new_selection, scene.selection.clone()),
                    )))
                    .unwrap()
            }
        }
    }

    fn open_material_editor(&mut self, material: Arc<Mutex<Material>>) {
        let engine = &mut self.engine;

        self.material_editor.set_material(Some(material), engine);

        engine.user_interface.send_message(WindowMessage::open(
            self.material_editor.window,
            MessageDirection::ToWidget,
            true,
        ));
    }

    fn update(&mut self, dt: f32) {
        scope_profile!();

        self.engine.update(dt);

        if let Mode::Play { scene, .. } = self.mode {
            self.engine.update_plugins(dt, true);

            self.engine.update_scene_scripts(scene, dt);
        }

        let mut needs_sync = false;

        while let Ok(message) = self.message_receiver.try_recv() {
            self.log.handle_message(&message, &mut self.engine);
            self.path_fixer
                .handle_message(&message, &self.engine.user_interface);

            if let Some(editor_scene) = self.scene.as_ref() {
                self.inspector
                    .handle_message(&message, editor_scene, &mut self.engine);
            }

            match message {
                Message::DoSceneCommand(command) => {
                    needs_sync |= self.do_scene_command(command);
                }
                Message::UndoSceneCommand => {
                    needs_sync |= self.undo_scene_command();
                }
                Message::RedoSceneCommand => {
                    needs_sync |= self.redo_scene_command();
                }
                Message::ClearSceneCommandStack => {
                    needs_sync |= self.clear_scene_command_stack();
                }
                Message::SelectionChanged => {
                    self.world_viewer.sync_selection = true;
                }
                Message::SyncToModel => {
                    needs_sync = true;
                }
                Message::SaveScene(path) => self.save_current_scene(path),
                Message::LoadScene(scene_path) => self.load_scene(scene_path),
                Message::SetInteractionMode(mode_kind) => {
                    self.set_interaction_mode(Some(mode_kind))
                }
                Message::Exit { force } => self.exit(force),
                Message::Log(msg) => {
                    println!("{}", msg)
                }
                Message::CloseScene => {
                    needs_sync |= self.close_current_scene();
                }
                Message::NewScene => self.create_new_scene(),
                Message::Configure { working_directory } => {
                    self.configure(working_directory);
                    needs_sync = true;
                }
                Message::OpenSettings(section) => {
                    self.menu.file_menu.settings.open(
                        &self.engine.user_interface,
                        &self.settings,
                        Some(section),
                    );
                }
                Message::OpenMaterialEditor(material) => self.open_material_editor(material),
                Message::ShowInAssetBrowser(path) => {
                    self.asset_browser
                        .locate_path(&self.engine.user_interface, path);
                }
                Message::SetWorldViewerFilter(filter) => {
                    self.world_viewer
                        .set_filter(filter, &self.engine.user_interface);
                }
                Message::LocateObject { type_id, handle } => {
                    self.world_viewer
                        .try_locate_object(type_id, handle, &self.engine)
                }
                Message::SelectObject { type_id, handle } => {
                    self.select_object(type_id, handle);
                }
                Message::SetEditorCameraProjection(projection) => {
                    if let Some(editor_scene) = self.scene.as_ref() {
                        editor_scene.camera_controller.set_projection(
                            &mut self.engine.scenes[editor_scene.scene].graph,
                            projection,
                        );
                    }
                }
                Message::SwitchMode => match self.mode {
                    Mode::Edit => self.set_play_mode(),
                    Mode::Play { .. } => self.set_editor_mode(),
                },
                Message::SwitchToPlayMode => self.set_play_mode(),
                Message::SwitchToEditMode => self.set_editor_mode(),
                Message::OpenLoadSceneDialog => {
                    self.menu
                        .open_load_file_selector(&mut self.engine.user_interface);
                }
            }
        }

        if needs_sync {
            self.sync_to_model();
        }

        self.handle_resize();

        if let Some(editor_scene) = self.scene.as_mut() {
            if self.mode.is_edit() {
                editor_scene.draw_debug(&mut self.engine, &self.settings.debugging);
            }

            let scene = &mut self.engine.scenes[editor_scene.scene];

            let camera = scene.graph[editor_scene.camera_controller.camera].as_camera_mut();

            camera
                .projection_mut()
                .set_z_near(self.settings.graphics.z_near);
            camera
                .projection_mut()
                .set_z_far(self.settings.graphics.z_far);

            let graph = &mut scene.graph;

            editor_scene.camera_controller.update(graph, dt);

            if let Some(mode) = self.current_interaction_mode {
                self.interaction_modes[mode as usize].update(
                    editor_scene,
                    editor_scene.camera_controller.camera,
                    &mut self.engine,
                );
            }

            self.asset_browser.update(&mut self.engine);
            self.material_editor.update(&mut self.engine);
        }
    }

    pub fn add_game_plugin<P: Plugin>(&mut self, plugin: P) {
        self.engine.add_plugin(plugin, true, false);
    }

    pub fn run(mut self, event_loop: EventLoop<()>) -> ! {
        event_loop.run(move |mut event, _, control_flow| {
            match event {
                Event::MainEventsCleared => {
                    update(&mut self);

                    if self.exit {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::RedrawRequested(_) => {
                    self.engine.render().unwrap();
                }
                Event::WindowEvent { ref event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            self.message_sender
                                .send(Message::Exit { force: false })
                                .unwrap();
                        }
                        WindowEvent::Resized(size) => {
                            if let Err(e) = self.engine.set_frame_size((*size).into()) {
                                fyrox::utils::log::Log::writeln(
                                    MessageKind::Error,
                                    format!("Failed to set renderer size! Reason: {:?}", e),
                                );
                            }
                            self.engine
                                .user_interface
                                .send_message(WidgetMessage::width(
                                    self.root_grid,
                                    MessageDirection::ToWidget,
                                    size.width as f32,
                                ));
                            self.engine
                                .user_interface
                                .send_message(WidgetMessage::height(
                                    self.root_grid,
                                    MessageDirection::ToWidget,
                                    size.height as f32,
                                ));
                        }
                        _ => (),
                    }

                    if let Some(os_event) = translate_event(event) {
                        self.engine.user_interface.process_os_event(&os_event);
                    }
                }
                Event::LoopDestroyed => {
                    if let Ok(profiling_results) = fyrox::core::profiler::print() {
                        if let Ok(mut file) =
                            fs::File::create(project_dirs::working_data_dir("profiling.log"))
                        {
                            let _ = writeln!(file, "{}", profiling_results);
                        }
                    }
                }
                _ => *control_flow = ControlFlow::Poll,
            }

            if let Mode::Play { scene, .. } = self.mode {
                let screen_bounds = self.scene_viewer.frame_bounds(&self.engine.user_interface);

                normalize_os_event(&mut event, screen_bounds.position, screen_bounds.size);

                self.engine
                    .handle_os_event_by_plugins(&event, FIXED_TIMESTEP, true);

                self.engine
                    .handle_os_event_by_scripts(&event, scene, FIXED_TIMESTEP);
            }
        });
    }
}

fn poll_ui_messages(editor: &mut Editor) {
    scope_profile!();

    while let Some(ui_message) = editor.engine.user_interface.poll_message() {
        editor.handle_ui_message(&ui_message);
    }
}

fn update(editor: &mut Editor) {
    scope_profile!();

    let mut dt =
        editor.game_loop_data.clock.elapsed().as_secs_f32() - editor.game_loop_data.elapsed_time;
    while dt >= FIXED_TIMESTEP {
        dt -= FIXED_TIMESTEP;
        editor.game_loop_data.elapsed_time += FIXED_TIMESTEP;

        editor.update(FIXED_TIMESTEP);

        poll_ui_messages(editor);

        editor.post_update();

        if dt >= 1.5 * FIXED_TIMESTEP {
            break;
        }
    }

    let window = editor.engine.get_window();
    window.set_cursor_icon(translate_cursor_icon(editor.engine.user_interface.cursor()));
    window.request_redraw();
}
