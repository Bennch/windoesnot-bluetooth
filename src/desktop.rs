use adw::prelude::*;
// WinDoesNot desktop integration: accent colour, light/dark mode and UI
// language follow the shared desktop preferences
// (`~/.config/quickshell/default/settings.txt`) when they exist.
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::OnceLock;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

pub const DEFAULT_ACCENT: &str = "#fe6f2d";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    De,
    En,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopPrefs {
    /// Validated `#rrggbb` accent colour.
    pub accent: Option<String>,
    pub dark: Option<bool>,
    pub language: Option<Language>,
    /// Corner radius in px, same setting Settings/Store use everywhere else.
    pub radius: Option<u32>,
}

pub fn settings_path() -> PathBuf {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("/nonexistent"));
    config.join("quickshell/default/settings.txt")
}

pub fn read_prefs() -> DesktopPrefs {
    std::fs::read_to_string(settings_path())
        .map(|text| parse_prefs(&text))
        .unwrap_or_default()
}

fn parse_prefs(text: &str) -> DesktopPrefs {
    let mut prefs = DesktopPrefs::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "accentColor" if is_hex_colour(value) => prefs.accent = Some(value.to_ascii_lowercase()),
            "darkMode" => prefs.dark = Some(value != "false"),
            "uiLanguage" => prefs.language = language_from(value),
            "borderRadius" => prefs.radius = value.parse().ok(),
            _ => {}
        }
    }
    prefs
}

fn is_hex_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn language_from(value: &str) -> Option<Language> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty() || value == "c" || value == "posix" {
        None
    } else if value.starts_with("de") {
        Some(Language::De)
    } else {
        Some(Language::En)
    }
}

fn system_language() -> Language {
    // LANGUAGE may list several languages ("de:en"); the first one wins.
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(var) {
            if let Some(language) = language_from(value.split(':').next().unwrap_or("")) {
                return language;
            }
        }
    }
    Language::En
}

/// Interface language for this run: the WinDoesNot setting, else the system locale.
/// Labels are built once, so a language change applies after restarting the store.
pub fn language() -> Language {
    static LANGUAGE: OnceLock<Language> = OnceLock::new();
    *LANGUAGE.get_or_init(|| read_prefs().language.unwrap_or_else(system_language))
}

pub fn accent() -> String {
    read_prefs().accent.unwrap_or_else(|| DEFAULT_ACCENT.to_string())
}

pub fn accent_rgb(accent: &str) -> (u8, u8, u8) {
    let channel = |i: usize| u8::from_str_radix(&accent[i..i + 2], 16).unwrap_or(0);
    (channel(1), channel(3), channel(5))
}

/// Readable text colour on top of the accent (WCAG relative luminance).
pub fn accent_foreground(accent: &str) -> &'static str {
    let (r, g, b) = accent_rgb(accent);
    let linear = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let luminance = 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
    if luminance > 0.45 { "#17120e" } else { "#ffffff" }
}

thread_local! {
    static MONITOR: RefCell<Option<gio::FileMonitor>> = RefCell::new(None);
    static PENDING: RefCell<Option<glib::SourceId>> = RefCell::new(None);
}

/// Calls `on_change` (debounced) whenever the desktop preferences file changes.
/// The directory is watched because the desktop may replace the file on save.
pub fn watch<F: Fn() + 'static>(on_change: F) {
    let path = settings_path();
    let Some(directory) = path.parent() else {
        return;
    };
    let Ok(monitor) = gio::File::for_path(directory)
        .monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
    else {
        return;
    };
    let on_change = std::rc::Rc::new(on_change);
    monitor.connect_changed(move |_, file, other, _| {
        let touches_settings = [Some(file), other]
            .into_iter()
            .flatten()
            .any(|candidate| candidate.basename().as_deref() == Some(std::path::Path::new("settings.txt")));
        if !touches_settings {
            return;
        }
        let on_change = on_change.clone();
        PENDING.with(|pending| {
            if let Some(id) = pending.borrow_mut().take() {
                id.remove();
            }
            let id = glib::timeout_add_local_once(std::time::Duration::from_millis(150), move || {
                PENDING.with(|pending| pending.borrow_mut().take());
                on_change();
            });
            *pending.borrow_mut() = Some(id);
        });
    });
    MONITOR.with(|slot| *slot.borrow_mut() = Some(monitor));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_preferences() {
        let prefs = parse_prefs("accentColor=#3A86FF\ndarkMode=false\nuiLanguage=de\nother=x\n");
        assert_eq!(prefs.accent.as_deref(), Some("#3a86ff"));
        assert_eq!(prefs.dark, Some(false));
        assert_eq!(prefs.language, Some(Language::De));
        assert_eq!(parse_prefs("accentColor=red;}\n").accent, None);
        assert_eq!(parse_prefs("uiLanguage=en\n").language, Some(Language::En));
    }

    #[test]
    fn foreground_stays_readable() {
        assert_eq!(accent_foreground("#fe6f2d"), "#ffffff");
        assert_eq!(accent_foreground("#ffe14d"), "#17120e");
        assert_eq!(accent_rgb("#fe6f2d"), (254, 111, 45));
    }
}


pub fn sync_color_scheme() {
    adw::StyleManager::default().set_color_scheme(match read_prefs().dark {
        Some(true) => adw::ColorScheme::ForceDark,
        Some(false) => adw::ColorScheme::ForceLight,
        None => adw::ColorScheme::Default,
    });
}

/// Shared WinDoesNot colour and control contract (same palette as Store/Settings).
pub fn setup() {
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(&gtk::gdk::Display::default().expect("display"), &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 50);
    let refresh = move || {
        let prefs = read_prefs();
        if std::env::var_os("WINDOESNOT_STYLE_DEBUG").is_some() { eprintln!("WDN_STYLE {:?} {:?}", settings_path(), prefs); }
        let manager = adw::StyleManager::default();
        manager.set_color_scheme(match prefs.dark {
            Some(true) => adw::ColorScheme::ForceDark,
            Some(false) => adw::ColorScheme::ForceLight,
            None => adw::ColorScheme::Default,
        });
        let (bg,sidebar,card,border,fg,selected) = if manager.is_dark() {
            ("#202020","#161616","#171717","#3a3a3a","#ffffff","#2a2a2a")
        } else { ("#f2f2f2","#e6e6e6","#f5f5f5","#d0d0d0","#1a1a1a","#c8c8c8") };
        let accent = prefs.accent.unwrap_or_else(|| DEFAULT_ACCENT.into());
        // Same default and same value everywhere as Settings' own borderRadius
        // (Settings.qml: `property int borderRadius: 10`, applied uniformly to
        // cards, rows and buttons there too).
        let radius = prefs.radius.unwrap_or(10);
        let css = format!(r#"
@define-color window_bg_color {bg}; @define-color window_fg_color {fg};
@define-color view_bg_color {bg}; @define-color view_fg_color {fg};
@define-color headerbar_bg_color {bg}; @define-color headerbar_fg_color {fg};
@define-color sidebar_bg_color {sidebar}; @define-color sidebar_fg_color {fg};
@define-color card_bg_color {card}; @define-color accent_bg_color {accent};
@define-color accent_color {accent}; @define-color accent_fg_color {accent_fg};
:root {{ --window-bg-color:{bg}; --window-fg-color:{fg}; --view-bg-color:{bg}; --view-fg-color:{fg};
 --headerbar-bg-color:{bg}; --headerbar-fg-color:{fg}; --sidebar-bg-color:{sidebar}; --sidebar-fg-color:{fg};
 --card-bg-color:{card}; --accent-bg-color:{accent}; --accent-color:{accent}; --accent-fg-color:{accent_fg}; }}
window, .background {{ background-color:{bg}; color:{fg}; font-family:Sans; font-size:14px; }}
headerbar {{ background:{bg}; box-shadow:none; border-bottom:1px solid {border}; }}
.wdn-sidebar, .sidebar {{ background:{sidebar}; }}
.wdn-brand {{ padding:40px 20px 30px; }}
.wdn-brand label {{ font-size:18px; font-weight:700; }}
.wdn-navigation {{ background:transparent; border:none; padding:0 10px; }}
.wdn-navigation row {{ min-height:45px; margin:0 0 8px 0; padding:0; border-radius:6px; background:transparent; box-shadow:none; outline:none; }}
.wdn-navigation row:hover {{ background:alpha({fg},0.07); }}
.wdn-navigation row:selected, .wdn-navigation row:selected:hover {{ background:{selected}; color:{fg}; box-shadow:none; }}
.wdn-nav-row {{ padding:0 10px 0 4px; }}
.wdn-nav-indicator {{ border-radius:2px; background:transparent; }}
.wdn-navigation row:selected .wdn-nav-indicator {{ background:{accent}; }}
/* Only the rows are cards; no second frame around the whole list. */
.boxed-list {{ border:none; background:transparent; box-shadow:none; }}
.boxed-list > row {{ margin:4px 0; border-radius:{radius}px; background:{card}; border:1px solid {border}; box-shadow:none; }}
.boxed-list > row:hover {{ background:alpha({fg},0.06); }}
.card {{ border-radius:{radius}px; background:{card}; border-color:{border}; box-shadow:none; }}
button {{ border-radius:{radius}px; box-shadow:none; }}
/* The value shown inside a drop-down row is a one-row list. The card rule for
   `.boxed-list row` also hit that inner row and framed it. */
row.combo listview.inline, row.combo listview.inline > row, row.combo listview.inline row {{
  margin:0; padding:0; min-height:0; border:none; border-radius:0;
  background:transparent; background-color:transparent; box-shadow:none; outline:none; }}
row.combo listview.inline row:hover {{ background:transparent; }}
/* Popups (drop-down lists, context menus): plain rows, not the cards used in pages. */
/* The window rule above also paints the popup's outer node (it has the class
   "background"): that was the rectangle behind the menu. Only `contents` is drawn. */
popover, popover.background, popover.menu, popover.background.menu {{ background:transparent; background-color:transparent; border:none; box-shadow:none; padding:0; }}
popover > contents {{ background:{card}; border:1px solid {border}; border-radius:{radius}px; padding:4px; box-shadow:0 6px 24px alpha(black,0.25); }}
popover contents > *, popover listview, popover list, popover scrolledwindow, popover scrolledwindow > *, popover box {{ background:transparent; background-color:transparent; border:none; box-shadow:none; outline:none; }}
popover row, popover .boxed-list row, popover listview > row, popover list > row {{
  margin:2px 0; padding:0 10px; min-height:36px; border-radius:6px;
  background:transparent; border:none; box-shadow:none; outline:none; color:{fg}; }}
popover row:hover {{ background:alpha({fg},0.08); }}
popover row:selected, popover row:selected:hover {{ background:alpha({accent},0.22); color:{fg}; }}
popover row label {{ margin:0; }}
popover.menu button {{ border-radius:6px; padding:8px 12px; }}
popover.menu button:hover {{ background:alpha({fg},0.08); }}
button.suggested-action {{ background:{accent}; color:{accent_fg}; }}
"#, accent_fg=accent_foreground(&accent));
        provider.load_from_data(&css);
    };
    refresh();
    watch(refresh);
}

/// Sidebar entry as in the Store: accent bar (selected only), icon, label.
pub fn nav_row(icon_name: &str, title: &str) -> gtk::ListBoxRow {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    content.add_css_class("wdn-nav-row");
    let indicator = gtk::Box::new(gtk::Orientation::Vertical, 0);
    indicator.add_css_class("wdn-nav-indicator");
    indicator.set_size_request(3, 22);
    indicator.set_valign(gtk::Align::Center);
    let icon = gtk::Image::from_icon_name(icon_name);
    icon.set_pixel_size(20);
    let label = gtk::Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    content.append(&indicator);
    content.append(&icon);
    content.append(&label);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&content));
    row
}

pub fn brand(title: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("wdn-brand");
    let picture = gtk::Picture::new();
    picture.set_size_request(118, 30);
    picture.set_can_shrink(true);
    picture.set_halign(gtk::Align::Start);
    picture.set_valign(gtk::Align::Center);
    let manager = adw::StyleManager::default();
    let update = |picture: &gtk::Picture, dark| {
        let bytes: &'static [u8] = if dark { include_bytes!("../assets/brand/windoesnot-logo-hell.png") }
            else { include_bytes!("../assets/brand/windoesnot-logo-dunkel.png") };
        if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(bytes)) {
            let height = (pixbuf.height() * 118 / pixbuf.width()).max(1);
            if let Some(scaled) = pixbuf.scale_simple(118, height, gtk::gdk_pixbuf::InterpType::Bilinear) {
                picture.set_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&scaled)));
            }
        }
    };
    update(&picture, manager.is_dark());
    let weak = picture.downgrade();
    manager.connect_dark_notify(move |manager| { if let Some(picture)=weak.upgrade() { update(&picture,manager.is_dark()); } });
    row.append(&picture);
    let label = gtk::Label::new(Some(title));
    row.append(&label);
    row
}

pub fn tr(english: &'static str) -> &'static str {
    if language() != Language::De { return english; }
    match english {
        "Bluetooth unavailable" => "Bluetooth nicht verfügbar",
        "No Bluetooth adapter or service is available. Check that your adapter is connected and Bluetooth is enabled." => "Kein Bluetooth-Adapter oder Dienst verfügbar. Prüfe, ob dein Adapter angeschlossen und Bluetooth eingeschaltet ist.",
        "Wi-Fi" => "WLAN", "Network" => "Netzwerk", "Devices" => "Geräte", "Profiles" => "Profile",
        "Settings" => "Einstellungen", "About" => "Über", "Appearance" => "Darstellung",
        "System" => "System", "Light" => "Hell", "Dark" => "Dunkel", "Cancel" => "Abbrechen",
        "Close" => "Schließen", "Apply" => "Übernehmen", "Apply Changes" => "Änderungen übernehmen",
        "Save" => "Speichern", "Delete" => "Löschen", "Remove" => "Entfernen", "Connect" => "Verbinden",
        "Disconnect" => "Trennen", "Connected" => "Verbunden", "Disconnected" => "Nicht verbunden",
        "Connecting…" => "Verbindung wird hergestellt …", "Connecting..." => "Verbindung wird hergestellt …",
        "Checking status…" => "Status wird geprüft …", "Scanning..." => "Suche läuft …", "Scan" => "Suchen",
        "Password" => "Passwort", "Show Password" => "Passwort anzeigen", "Refresh" => "Aktualisieren",
        "No Networks Available" => "Keine Netzwerke verfügbar", "No networks found" => "Keine Netzwerke gefunden",
        "Available Networks" => "Verfügbare Netzwerke", "Saved Networks" => "Gespeicherte Netzwerke",
        "Known Networks" => "Bekannte Netzwerke", "Hidden Network" => "Verborgenes Netzwerk",
        "Connect to Hidden Network" => "Mit verborgenem Netzwerk verbinden", "Forget" => "Vergessen",
        "General Bluetooth Settings" => "Allgemeine Bluetooth-Einstellungen", "Bluetooth Settings" => "Bluetooth-Einstellungen",
        "All the devices you've connected to" => "Bereits gekoppelte Geräte", "No devices in range" => "Keine Geräte in Reichweite",
        "Device" => "Gerät", "Adapter" => "Adapter", "Adapters" => "Adapter", "Pair" => "Koppeln",
        "Paired" => "Gekoppelt", "Trust" => "Vertrauen", "Trusted" => "Vertrauenswürdig", "Block" => "Blockieren",
        "Blocked" => "Blockiert", "Unblock" => "Freigeben", "Rename" => "Umbenennen", "Send File" => "Datei senden",
        "File Transfers" => "Dateiübertragungen", "Received Files" => "Empfangene Dateien", "Discoverable" => "Sichtbar",
        "Discovering" => "Suche läuft", "More Information" => "Weitere Informationen", "Battery" => "Akku",
        "Connection status" => "Verbindungsstatus", "Settings & About" => "Einstellungen und Informationen",
        "Hotspot active" => "Hotspot aktiv", "Behavior" => "Verhalten", "Modules" => "Bereiche",
        "Reset" => "Zurücksetzen", "No network hardware detected. Check your hardware or restart NetworkManager." =>
            "Keine Netzwerkgeräte erkannt. Prüfe die Hardware und den NetworkManager-Dienst.",
        _ => english,
    }
}
