//! Alternate YAML format for tmTheme files, because that XML is
//! annoying and hard to edit.
//!
//! See `emma.theme.yml` for an example.

use anyhow::{anyhow, bail, Error};
use fehler::throws;
use serde::Deserialize;
use std::collections::HashMap;
use syntect::highlighting::{
    Color, ScopeSelector, ScopeSelectors, StyleModifier, Theme, ThemeItem,
};
use syntect::parsing::{Scope, ScopeStack};

#[derive(Debug, Deserialize)]
struct YamlThemeSettings {
    foreground: Option<String>,
    background: Option<String>,
}

#[derive(Debug, Deserialize)]
struct YamlThemeScope {
    scope: String,
    foreground: Option<String>,
    background: Option<String>,
}

impl YamlThemeScope {
    #[throws]
    fn scope_selectors(&self) -> Vec<ScopeSelector> {
        // TODO: it seems likely from the complexity of the types here
        // that there are more complex ways to specify scopes than
        // what we currently have (e.g. excludes and scope stacks), so
        // more parsing stuff is probably needed here.
        self.scope
            .split(",")
            .map(|s| -> Result<ScopeSelector, Error> {
                Ok(ScopeSelector {
                    path: ScopeStack::from_vec(vec![Scope::new(s.trim())
                        .map_err(|e| anyhow!("ParseScopeError({:?})", e))?]),
                    excludes: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?
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
    #[throws]
    fn expand_vars(&mut self) {
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

        expand(&mut self.settings.foreground)?;
        for scope in self.scopes.values_mut() {
            expand(&mut scope.foreground)?;
        }
    }
}

#[throws]
fn parse_color(s: &Option<String>) -> Option<Color> {
    if let Some(s) = s {
        if s.starts_with('#') {
            // TODO: support shorthand colors like "#555"?
            let rest = &s[1..];
            if rest.len() == 6 {
                // TODO: alpha support
                Some(Color {
                    r: u8::from_str_radix(&rest[0..2], 16)?,
                    g: u8::from_str_radix(&rest[2..4], 16)?,
                    b: u8::from_str_radix(&rest[4..6], 16)?,
                    a: 255,
                })
            } else {
                bail!("color is too short: {}", s);
            }
        } else {
            bail!("color does not start with '#': {}", s);
        }
    } else {
        None
    }
}

#[throws]
fn load_theme(theme: &str) -> Theme {
    let mut yaml: YamlTheme = serde_yaml::from_str(&theme)?;
    yaml.expand_vars()?;

    let mut theme = Theme::default();
    theme.name = Some(yaml.name);

    theme.settings.foreground = parse_color(&yaml.settings.foreground)?;
    for scope in yaml.scopes.values() {
        theme.scopes.push(ThemeItem {
            scope: ScopeSelectors {
                selectors: scope.scope_selectors()?,
            },
            style: StyleModifier {
                foreground: parse_color(&scope.foreground)?,
                background: parse_color(&scope.background)?,
                // TODO
                font_style: None,
            },
        });
    }

    theme
}

#[throws]
pub fn load_default_theme() -> Theme {
    let theme = include_str!("emma.theme.yml");
    load_theme(theme)?
}
