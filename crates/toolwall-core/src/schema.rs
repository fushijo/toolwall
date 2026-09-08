//! Typed mirror of `schema/toolwall.schema.json`.
//!
//! The JSON Schema is the published contract; these types are the Rust view of
//! it. When you change one, change the other and regenerate the example.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub version: u32,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,

    #[serde(default)]
    pub input: Input,

    #[serde(default)]
    pub theme: Theme,

    #[serde(default)]
    pub window: Window,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Experimental>,

    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shaders: std::collections::BTreeMap<String, Shader>,

    /// Mode applied on startup. `None` starts unmodified.
    #[serde(default)]
    pub default_mode: Option<String>,

    /// Scene object ids live in every mode.
    #[serde(default)]
    pub base_overlays: Vec<String>,

    #[serde(default)]
    pub modes: Vec<Mode>,

    #[serde(default)]
    pub mirrors: Vec<Mirror>,

    #[serde(default)]
    pub images: Vec<Image>,

    #[serde(default)]
    pub text: Vec<Text>,

    /// Extra programs hosted as floating windows, e.g. Ninjabrain Bot.
    #[serde(default)]
    pub apps: Vec<App>,

    #[serde(default)]
    pub keybinds: Vec<Keybind>,

    #[serde(default)]
    pub hud: Hud,

    #[serde(default)]
    pub gui: Gui,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            meta: None,
            input: Input::default(),
            theme: Theme::default(),
            window: Window::default(),
            experimental: None,
            shaders: Default::default(),
            default_mode: None,
            base_overlays: Vec::new(),
            modes: Vec::new(),
            mirrors: Vec::new(),
            images: Vec::new(),
            text: Vec::new(),
            apps: Vec::new(),
            keybinds: Vec::new(),
            hud: Hud::default(),
            gui: Gui::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub rules: String,
    #[serde(default)]
    pub variant: String,
    #[serde(default)]
    pub options: String,

    /// Source keycode or mouse button -> output keycode.
    #[serde(default)]
    pub remaps: std::collections::BTreeMap<String, String>,

    /// -1 inherits the host Wayland session.
    #[serde(default = "minus_one")]
    pub repeat_rate: i32,
    #[serde(default = "minus_one")]
    pub repeat_delay: i32,

    #[serde(default = "one_f64")]
    pub sensitivity: f64,
    #[serde(default)]
    pub confine_pointer: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            layout: String::new(),
            model: String::new(),
            rules: String::new(),
            variant: String::new(),
            options: String::new(),
            remaps: Default::default(),
            repeat_rate: -1,
            repeat_delay: -1,
            sensitivity: 1.0,
            confine_pointer: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    #[serde(default = "black")]
    pub background: String,
    #[serde(default)]
    pub background_png: String,
    #[serde(default)]
    pub cursor_theme: String,
    #[serde(default)]
    pub cursor_icon: String,
    #[serde(default)]
    pub cursor_size: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ninb_anchor: Option<NinbAnchor>,
    #[serde(default = "one_f64")]
    pub ninb_opacity: f64,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: "#000000ff".into(),
            background_png: String::new(),
            cursor_theme: String::new(),
            cursor_icon: String::new(),
            cursor_size: 0,
            ninb_anchor: None,
            ninb_opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NinbAnchor {
    Named(String),
    Offset {
        position: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<i32>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub fullscreen_width: u32,
    #[serde(default)]
    pub fullscreen_height: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Experimental {
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub jit: bool,
    #[serde(default)]
    pub tearing: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Shader {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mode {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub resolution: Resolution,
    /// `None` inherits `input.sensitivity`.
    #[serde(default)]
    pub sensitivity: Option<f64>,
    #[serde(default = "yes")]
    pub toggle: bool,
    #[serde(default)]
    pub mirrors: Vec<String>,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorKey {
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mirror {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub src: Rect,
    pub dst: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_key: Option<ColorKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub path: String,
    pub dst: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Text {
    pub id: String,
    /// Placeholders: `{mode} {res} {width} {height} {sens} {state}`.
    pub template: String,
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
}

/// A program launched into waywall as a floating window.
///
/// The command is split on spaces into argv by waywall, and is trusted the
/// same way `gui.command` is - it is named by the config owner, not by a
/// keybind, so a shared config still cannot smuggle in a command that runs
/// without you binding it to a key yourself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybind {
    /// waywall keysym with modifiers, e.g. `Ctrl-I`, `Shift-T`, `*-F3`.
    pub input: String,
    pub command: Command,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The closed set of verbs a keybind may invoke.
///
/// Keybinds never carry Lua source. A config downloaded from someone else
/// cannot execute arbitrary code on load, which is what makes sharing safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    #[serde(rename = "mode.set")]
    ModeSet,
    #[serde(rename = "mode.reset")]
    ModeReset,
    #[serde(rename = "mode.cycle")]
    ModeCycle,
    #[serde(rename = "sens.set")]
    SensSet,
    #[serde(rename = "keymap.set")]
    KeymapSet,
    #[serde(rename = "remaps.set")]
    RemapsSet,
    #[serde(rename = "key.press")]
    KeyPress,
    #[serde(rename = "fullscreen.toggle")]
    FullscreenToggle,
    #[serde(rename = "floating.toggle")]
    FloatingToggle,
    #[serde(rename = "floating.show")]
    FloatingShow,
    #[serde(rename = "floating.hide")]
    FloatingHide,
    #[serde(rename = "gui.toggle")]
    GuiToggle,
    #[serde(rename = "app.toggle")]
    AppToggle,
    #[serde(rename = "exec")]
    Exec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hud {
    #[serde(default = "base_label")]
    pub idle_label: String,
    /// Requires the State Output mod in the instance.
    #[serde(default)]
    pub follow_state: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<Text>,
}

impl Default for Hud {
    fn default() -> Self {
        Self { idle_label: "base".into(), follow_state: false, banner: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gui {
    #[serde(default = "gui_command")]
    pub command: String,
    #[serde(default = "launch_delay")]
    pub launch_delay_ms: u32,
    /// Enables the arbitrary `exec` keybind command. Off by default.
    #[serde(default)]
    pub allow_exec: bool,
    /// How the editor window itself looks. Edited from inside the editor.
    #[serde(default)]
    pub appearance: Appearance,
}

impl Default for Gui {
    fn default() -> Self {
        Self {
            command: "toolwall-gui".into(),
            launch_delay_ms: 400,
            allow_exec: false,
            appearance: Appearance::default(),
        }
    }
}

/// The editor's own look. Kept in the same document as everything else so it
/// survives a restart and travels with a shared config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    /// Window opacity. The editor floats over the game, so seeing through it
    /// matters more here than in a normal desktop app.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default = "yes")]
    pub dark: bool,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { opacity: 0.92, dark: true, font_size: 14.0 }
    }
}

fn minus_one() -> i32 { -1 }
fn one_f64() -> f64 { 1.0 }
fn yes() -> bool { true }
fn black() -> String { "#000000ff".into() }
fn base_label() -> String { "base".into() }
fn gui_command() -> String { "toolwall-gui".into() }
fn launch_delay() -> u32 { 400 }
fn default_opacity() -> f32 { 0.92 }
fn default_font_size() -> f32 { 14.0 }
