use crate::InputState;
use crate::engine::{Engine, LayoutError};
use crate::geometry::Size;
use crate::id::ElementId;
use crate::node::Node;
use crate::result::LayoutResult;
use crate::style::TextStyle;

/// Immediate-mode frame builder.
///
/// A frame starts with an implicit root node sized to the viewport. Open nodes,
/// push children, close nodes, then call [`Frame::end`] to run layout.
pub struct Frame<'a> {
    pub(crate) engine: &'a mut Engine,
    pub(crate) size: Size,
    pub(crate) stack: Vec<Node>,
}

impl Frame<'_> {
    /// Opens a node and makes it the current parent.
    pub fn open(&mut self, node: Node) {
        self.stack.push(node);
    }

    /// Appends a text node to the current parent.
    pub fn text(&mut self, text: impl Into<String>, style: TextStyle) {
        self.child(Node::text(text, style));
    }

    /// Appends a child node to the current parent.
    pub fn child(&mut self, node: Node) {
        if let Some(parent) = self.stack.last_mut() {
            parent.children.push(node);
        }
    }

    /// Returns the stable id of the currently open node.
    #[must_use]
    pub fn open_element_id(&self) -> Option<&ElementId> {
        self.stack.last().and_then(|node| node.element_id.as_ref())
    }

    /// Returns whether the currently open node was hovered in a previous result.
    #[must_use]
    pub fn hovered(&self, result: &LayoutResult) -> bool {
        self.stack
            .last()
            .and_then(|node| node.id.as_deref())
            .is_some_and(|id| result.pointer_over(id))
    }

    /// Closes the current node and appends it to its parent.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::UnbalancedClose`] when called at the root level.
    pub fn close(&mut self) -> Result<(), LayoutError> {
        if self.stack.len() <= 1 {
            return Err(LayoutError::UnbalancedClose);
        }
        if let Some(node) = self.stack.pop() {
            self.child(node);
        }
        Ok(())
    }

    /// Finishes the frame and returns layout output.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::UnclosedElements`] when one or more nodes are
    /// still open.
    pub fn end(mut self) -> Result<LayoutResult, LayoutError> {
        if self.stack.len() != 1 {
            return Err(LayoutError::UnclosedElements);
        }
        if let Some(root) = self.stack.pop() {
            Ok(self.engine.layout(&root, self.size))
        } else {
            Err(LayoutError::UnclosedElements)
        }
    }

    /// Returns mutable access to the engine input state while building a frame.
    pub fn input_state(&mut self) -> &mut InputState {
        self.engine.input_mut()
    }
}
