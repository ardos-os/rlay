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
/// push children, close nodes, then call [`Frame::end`] with delta time in
/// seconds to run layout.
pub struct Frame<'a> {
    pub(crate) engine: &'a mut Engine,
    pub(crate) size: Size,
    pub(crate) nodes: Vec<FrameNode>,
    pub(crate) stack: Vec<usize>,
}

pub(crate) struct FrameNode {
    pub(crate) node: Node,
    pub(crate) first_child: Option<usize>,
    pub(crate) last_child: Option<usize>,
    pub(crate) next_sibling: Option<usize>,
    pub(crate) inline_text: Option<(String, TextStyle)>,
    pub(crate) child_count: usize,
}

impl FrameNode {
    pub(crate) fn new(node: Node) -> Self {
        Self {
            node,
            first_child: None,
            last_child: None,
            next_sibling: None,
            inline_text: None,
            child_count: 0,
        }
    }
}

impl Frame<'_> {
    /// Opens a node and makes it the current parent.
    pub fn open(&mut self, node: Node) {
        let index = self.push_child(node);
        self.stack.push(index);
    }

    /// Appends a text node to the current parent.
    pub fn text(&mut self, text: impl Into<String>, style: TextStyle) {
        if let Some(parent_index) = self.stack.last().copied()
            && self.nodes[parent_index].child_count == 0
            && self.nodes[parent_index].inline_text.is_none()
        {
            self.nodes[parent_index].inline_text = Some((text.into(), style));
            return;
        }
        self.child(Node::text(text, style));
    }

    /// Appends a child node to the current parent.
    pub fn child(&mut self, node: Node) {
        self.push_child(node);
    }

    /// Returns the stable id of the currently open node.
    #[must_use]
    pub fn open_element_id(&self) -> Option<&ElementId> {
        self.stack
            .last()
            .and_then(|index| self.nodes.get(*index))
            .and_then(|node| node.node.element_id.as_ref())
    }

    /// Returns whether the currently open node was hovered in a previous result.
    #[must_use]
    pub fn hovered(&self, result: &LayoutResult) -> bool {
        self.stack
            .last()
            .and_then(|index| self.nodes.get(*index))
            .and_then(|node| node.node.id.as_deref())
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
        self.stack.pop();
        Ok(())
    }

    /// Finishes the frame and returns layout output.
    ///
    /// Negative and non-finite delta times are treated as zero.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::UnclosedElements`] when one or more nodes are
    /// still open.
    pub fn end(mut self, delta_time: f32) -> Result<LayoutResult, LayoutError> {
        if self.stack.len() != 1 {
            self.recycle();
            return Err(LayoutError::UnclosedElements);
        }
        if let Some(result) = self
            .engine
            .layout_frame(&mut self.nodes, self.size, delta_time)
        {
            self.recycle();
            return Ok(result);
        }
        let root = self.build_node_tree(0);
        self.recycle();
        Ok(self.engine.layout(&root, self.size, delta_time))
    }

    /// Returns mutable access to the engine input state while building a frame.
    pub fn input_state(&mut self) -> &mut InputState {
        self.engine.input_mut()
    }

    fn push_child(&mut self, node: Node) -> usize {
        self.flush_inline_text();
        let index = self.nodes.len();
        self.nodes.push(FrameNode::new(node));
        let Some(parent_index) = self.stack.last().copied() else {
            return index;
        };
        match self.nodes[parent_index].last_child {
            Some(last_child) => {
                self.nodes[last_child].next_sibling = Some(index);
            }
            None => {
                self.nodes[parent_index].first_child = Some(index);
            }
        }
        self.nodes[parent_index].last_child = Some(index);
        self.nodes[parent_index].child_count += 1;
        index
    }

    fn flush_inline_text(&mut self) {
        let Some(parent_index) = self.stack.last().copied() else {
            return;
        };
        let Some((text, style)) = self.nodes[parent_index].inline_text.take() else {
            return;
        };
        self.push_child_to_parent(parent_index, Node::text(text, style));
    }

    fn push_child_to_parent(&mut self, parent_index: usize, node: Node) -> usize {
        let index = self.nodes.len();
        self.nodes.push(FrameNode::new(node));
        match self.nodes[parent_index].last_child {
            Some(last_child) => {
                self.nodes[last_child].next_sibling = Some(index);
            }
            None => {
                self.nodes[parent_index].first_child = Some(index);
            }
        }
        self.nodes[parent_index].last_child = Some(index);
        self.nodes[parent_index].child_count += 1;
        index
    }

    fn build_node_tree(&mut self, index: usize) -> Node {
        let mut node = std::mem::take(&mut self.nodes[index].node);
        let has_inline_text = self.nodes[index].inline_text.is_some();
        node.children.reserve(self.nodes[index].child_count);
        if let Some((text, style)) = self.nodes[index].inline_text.take() {
            node.children.push(Node::text(text, style));
        }
        let mut child = self.nodes[index].first_child;
        while let Some(child_index) = child {
            node.children.push(self.build_node_tree(child_index));
            child = self.nodes[child_index].next_sibling;
        }
        if has_inline_text {
            debug_assert_eq!(node.children.len(), self.nodes[index].child_count + 1);
        }
        node
    }

    fn recycle(&mut self) {
        self.nodes.clear();
        self.stack.clear();
        self.engine.scratch_frame_nodes = std::mem::take(&mut self.nodes);
        self.engine.scratch_frame_stack = std::mem::take(&mut self.stack);
    }
}
