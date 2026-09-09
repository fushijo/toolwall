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

    #[serde(default)]
    pub ninb: Ninb,

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
            ninb: Ninb::default(),
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

    /// Remaps applied instead while the cursor is visible - inventory, a
    /// menu, or paused. Empty means "use `remaps` everywhere", so this costs
    /// nothing unless you opt in. Needs the State Output mod.
    #[serde(default)]
    pub remaps_menu: std::collections::BTreeMap<String, String>,

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
            remaps_menu: Default::default(),
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

/// Which corner a capture region is measured from.
///
/// Minecraft pins its debug HUD to the corners at fixed pixel offsets, not to
/// fractions of the window: the pie chart is always the same size and the same
/// distance from the bottom-right corner, whether you are at 340x1080 or
/// fullscreen. Anchoring reproduces that, so one capture is correct at every
/// resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Anchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    /// Centred on the crosshair. `src.x` / `src.y` are ignored.
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
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
    /// Corner that `src.x` / `src.y` are measured from. Without it they are
    /// absolute, which is only correct at one resolution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_anchor: Option<Anchor>,
    #[serde(default = "zero_rect")]
    pub src: Rect,
    pub dst: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_key: Option<ColorKey>,

    /// Several colour keys, drawn as one layer each.
    ///
    /// Colour keying passes only the matching colour, so isolating something
    /// many-coloured - the pie chart - means one layer per colour stacked up.
    /// This is the hot-reloadable alternative to a crop shader, which waywall
    /// only compiles at startup.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub color_keys: Vec<ColorKey>,
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

/// Ninjabrain Bot, hosted as a floating window over the game.
///
/// Deliberately a single named thing rather than a generic app list: it is
/// the only third-party window a run actually uses, and a list invites
/// configuring things that will not behave like ninb does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ninb {
    /// Path to the Ninjabrain Bot jar. Empty means "not set up".
    #[serde(default)]
    pub jar: String,
    /// Command used to launch it. `{jar}` is replaced with the path above.
    #[serde(default = "ninb_command")]
    pub command: String,
}

impl Default for Ninb {
    fn default() -> Self {
        Self { jar: String::new(), command: ninb_command() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybind {
    /// Suppress this bind while F3 is held, so F3 combos reach Minecraft.
    ///
    /// waywall matches modifiers exactly, so Shift, Ctrl and Alt are already
    /// safe. F3 is an ordinary key, so without this a bind on B also fires
    /// for F3+B.
    #[serde(default = "yes")]
    pub f3_safe: bool,
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
    #[serde(rename = "ninb.toggle")]
    NinbToggle,
    #[serde(rename = "overlay.toggle")]
    OverlayToggle,
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
        Self { opacity: 0.92, dark: true, font_size: 18.0 }
    }
}

fn minus_one() -> i32 { -1 }
fn one_f64() -> f64 { 1.0 }
fn yes() -> bool { true }
fn black() -> String { "#000000ff".into() }
fn base_label() -> String { "base".into() }
fn gui_command() -> String { "toolwall-gui".into() }
fn launch_delay() -> u32 { 400 }
fn zero_rect() -> Rect { Rect { x: 0, y: 0, w: 0, h: 0 } }
fn ninb_command() -> String { "java -jar {jar}".into() }
fn default_opacity() -> f32 { 0.92 }
fn default_font_size() -> f32 { 18.0 }
