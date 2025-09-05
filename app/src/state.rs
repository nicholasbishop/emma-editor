mod event;
mod persistence;

use crate::LineHeight;
use crate::buffer::{Buffer, BufferId};
use crate::overlay::Overlay;
use crate::pane_tree::PaneTree;
use crate::rope::AbsLine;
use crate::theme::Theme;
use crate::widget::Widget;
use anyhow::Result;
use persistence::PersistedBuffer;
use std::collections::HashMap;
use tracing::{error, info};

pub struct AppState {
    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    line_height: LineHeight,

    is_persistence_enabled: bool,

    overlay: Option<Overlay>,
}

impl AppState {
    pub fn buffers(&self) -> &HashMap<BufferId, Buffer> {
        &self.buffers
    }

    pub fn pane_tree(&self) -> &PaneTree {
        &self.pane_tree
    }

    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    pub fn line_height(&self) -> LineHeight {
        self.line_height
    }

    pub fn set_line_height(&mut self, line_height: LineHeight) {
        self.line_height = line_height;
    }

    pub fn enable_persistence(&mut self) {
        self.is_persistence_enabled = true;
    }

    pub fn recalc_layout(&mut self, width: f64, height: f64) {
        self.pane_tree.recalc_layout(width, height);

        // TODO: generalize this somehow.
        if let Some(overlay) = &mut self.overlay {
            overlay.recalc_layout(width, self.line_height);
        }
    }

    // TODO: for the persisted data, perhaps we want a trait to abstract
    // that instead of passing the data in.
    pub fn load(
        persisted_buffers: &[PersistedBuffer],
        pane_tree_json: Result<String>,
    ) -> Self {
        Theme::set_current(
            Theme::load_default().expect("failed to load built-in theme"),
        );

        // Always create an empty scratch buffer.
        let mut scratch_buffer = Buffer::create_empty();

        let mut buffers = HashMap::new();
        let mut cursors = HashMap::new();
        for pb in persisted_buffers {
            info!("loading {:?}", pb);
            cursors.insert(pb.buffer_id.clone(), pb.cursors.clone());
            // TODO; handle no path cases as well.
            if let Some(path) = &pb.path {
                buffers.insert(
                    pb.buffer_id.clone(),
                    Buffer::from_path(path).unwrap(),
                );
            }
        }

        let mut pane_tree = match pane_tree_json
            .and_then(|json| PaneTree::load_from_json(&json))
        {
            Ok(pt) => pt,
            Err(err) => {
                error!("failed to load persisted pane tree: {}", err);
                PaneTree::new(&mut scratch_buffer)
            }
        };

        let scratch_buffer_id = scratch_buffer.id().clone();
        buffers.insert(scratch_buffer_id.clone(), scratch_buffer);

        // Ensure that all the panes are pointing to a valid buffer.
        for pane in pane_tree.panes_mut() {
            if let Some(buffer) = buffers.get_mut(pane.buffer_id()) {
                // Default the cursor to the top of the buffer, then try to
                // restore the proper location from persisted data.
                buffer.set_cursor(pane.id(), Default::default());
                if let Some(cursors) = cursors.get(pane.buffer_id())
                    && let Some(pane_cursor) = cursors.get(pane.id())
                {
                    buffer.set_cursor(pane.id(), *pane_cursor);
                }
            } else {
                pane.switch_buffer(&mut buffers, &scratch_buffer_id);
            }

            // Ensure that the pane's top-line is valid.
            let buffer = buffers.get(pane.buffer_id()).unwrap();
            if pane.top_line() >= AbsLine(buffer.text().len_lines()) {
                pane.set_top_line(AbsLine(0));
            }
        }

        Self {
            key_handler: event::KeyHandler::new().unwrap(),

            buffers,
            pane_tree,

            // Outside of tests this is overwritten with a
            // dynamically-calculated value later.
            line_height: LineHeight(20.0),

            is_persistence_enabled: false,
            overlay: None,
        }
    }
}
