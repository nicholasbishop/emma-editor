use super::App;
use anyhow::Result;

impl App {
    pub fn persistence_store(&self) -> Result<()> {
        let json = serde_json::to_string(&self.pane_tree)?;
        dbg!(json);
        Ok(())
    }
}
