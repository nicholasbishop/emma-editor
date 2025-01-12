//! Alternate YAML format for tmTheme files, because that XML is
//! annoying and hard to edit.
//!
//! See `emma.theme.yml` for an example.

use anyhow::{anyhow, bail, Error, Result};
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use syntect::highlighting::{
    Color, ParseThemeError, ScopeSelectors, StyleModifier,
    Theme as SyntectTheme, ThemeItem,
};
use syntect::LoadingError;

static THEME: Lazy<Arc<Mutex<Option<Theme>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

#[derive(Debug, Deserialize)]
struct YamlThemeItem {
    foreground: Option<String>,
    background: Option<String>,
    // TODO: may add font settings here
}

#[derive(Debug, Deserialize)]
struct YamlThemeSettings {
    caret: Option<String>,
    foreground: Option<String>,
    background: Option<String>,
    info_bar_active: Option<YamlThemeItem>,
    info_bar_inactive: Option<YamlThemeItem>,
    search_match: Option<YamlThemeItem>,
}

#[derive(Debug, Deserialize)]
struct YamlThemeScope {
    scope: String,
    foreground: Option<String>,
    background: Option<String>,
}

impl YamlThemeScope {
    fn scope_selectors(&self) -> Result<ScopeSelectors, ParseThemeError> {
        Ok(self.scope.parse()?)
    }
}

#[derive(Debug, Deserialize)]
struct YamlTheme {
    name: String,
    settings: YamlThemeSettings,
    vars: HashMap<String, String>,
    scopes: HashMap<String, YamlThemeScope>,
}

impl YamlTheme {
    fn expand_vars(&mut self) -> Result<()> {
        // For now variable expansion is very simple. It only works
        // for colors, and if a variable is used it must be the entire
        // string, e.g. you can have "$myvar" but not "foo$myvar".

        let vars = &self.vars;

        let expand = |s: &mut Option<String>| {
            if let Some(s) = s {
                if s.starts_with('$') {
                    let name = &s[1..];
                    if let Some(value) = vars.get(name) {
                        *s = value.clone();
                    } else {
                        return Err(anyhow!("invalid variable: {}", name));
                    }
                }
            }
            Ok(())
        };

        let expand_item =
            |item: &mut Option<YamlThemeItem>| -> Result<_, Error> {
                if let Some(item) = item {
                    expand(&mut item.foreground)?;
                    expand(&mut item.background)?;
                }
                Ok(())
            };

        expand(&mut self.settings.caret)?;
        expand(&mut self.settings.foreground)?;
        expand(&mut self.settings.background)?;

        expand_item(&mut self.settings.info_bar_active)?;
        expand_item(&mut self.settings.info_bar_inactive)?;

        expand_item(&mut self.settings.search_match)?;

        for scope in self.scopes.values_mut() {
            expand(&mut scope.foreground)?;
        }

        Ok(())
    }
}

#[allow(clippy::many_single_char_names)]
fn parse_color(s: &Option<String>) -> Result<Option<Color>> {
    if let Some(s) = s {
        if let Some(rest) = s.strip_prefix('#') {
            // TODO: support shorthand colors like "#555"?
            if rest.len() >= 6 {
                let r = u8::from_str_radix(&rest[0..2], 16)?;
                let g = u8::from_str_radix(&rest[2..4], 16)?;
                let b = u8::from_str_radix(&rest[4..6], 16)?;

                let a = if rest.len() == 6 {
                    255
                } else if rest.len() == 8 {
                    u8::from_str_radix(&rest[4..6], 16)?
                } else {
                    bail!("color has invalid length: {}", s);
                };

                Ok(Some(Color { r, g, b, a }))
            } else {
                bail!("color is too short: {}", s);
            }
        } else {
            bail!("color does not start with '#': {}", s);
        }
    } else {
        Ok(None)
    }
}

#[derive(Clone, Debug)]
pub struct ForeAndBack {
    pub foreground: Color,
    pub background: Color,
    // TODO: maybe add font options
}

impl ForeAndBack {
    fn parse_with_default(
        item: &Option<YamlThemeItem>,
        foreground: Color,
        background: Color,
    ) -> Result<Self> {
        if let Some(item) = item {
            Ok(Self {
                foreground: parse_color(&item.foreground)?
                    .unwrap_or(foreground),
                background: parse_color(&item.background)?
                    .unwrap_or(background),
            })
        } else {
            Ok(Self {
                foreground,
                background,
            })
        }
    }
}

#[derive(Clone, Debug)]
pub struct Theme {
    pub syntect: SyntectTheme,
    pub info_bar_active: ForeAndBack,
    pub info_bar_inactive: ForeAndBack,
    pub search_match: ForeAndBack,
}

impl Theme {
    pub fn set_current(theme: Self) {
        let mut guard = THEME.lock().unwrap();
        *guard = Some(theme);
    }

    pub fn current() -> Self {
        THEME.lock().unwrap().clone().unwrap()
    }

    fn load(theme: &str) -> Result<Self> {
        let mut yaml: YamlTheme = serde_yaml::from_str(theme)?;
        yaml.expand_vars()?;

        let mut theme = SyntectTheme {
            name: Some(yaml.name),
            ..SyntectTheme::default()
        };

        theme.settings.caret = parse_color(&yaml.settings.caret)?;
        theme.settings.foreground = parse_color(&yaml.settings.foreground)?;
        theme.settings.background = parse_color(&yaml.settings.background)?;
        for scope in yaml.scopes.values() {
            theme.scopes.push(ThemeItem {
                scope: scope.scope_selectors().map_err(LoadingError::from)?,
                style: StyleModifier {
                    foreground: parse_color(&scope.foreground)?,
                    background: parse_color(&scope.background)?,
                    // TODO
                    font_style: None,
                },
            });
        }

        Ok(Self {
            syntect: theme,
            info_bar_active: ForeAndBack::parse_with_default(
                &yaml.settings.info_bar_active,
                rgb(255, 255, 255),
                rgb(255, 0, 0),
            )?,
            info_bar_inactive: ForeAndBack::parse_with_default(
                &yaml.settings.info_bar_inactive,
                rgb(0, 0, 0),
                rgb(255, 128, 128),
            )?,
            search_match: ForeAndBack::parse_with_default(
                &yaml.settings.search_match,
                rgb(0, 0, 0),
                rgb(255, 128, 128),
            )?,
        })
    }

    pub fn load_default() -> Result<Self> {
        let theme = include_str!("emma.theme.yml");
        Self::load(theme)
    }
}
