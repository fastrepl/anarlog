use std::{ops::Range, sync::Arc};

use desktop_runtime::{Result, ServiceError};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Window,
    Dialog,
    Button,
    TextInput,
    Document,
    List,
    ListItem,
    Status,
    Alert,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub role: Role,
    pub name: Arc<str>,
    pub description: Option<Arc<str>>,
    pub children: Arc<[NodeId]>,
    pub disabled: bool,
    pub selected: bool,
    pub expanded: Option<bool>,
    pub text: Option<Arc<str>>,
    pub selection_utf16: Option<Range<usize>>,
}

#[derive(Clone, Debug)]
pub enum Action {
    Focus(NodeId),
    Activate(NodeId),
    SetSelection(NodeId, Range<usize>),
    ReplaceText(NodeId, Arc<str>),
}

pub trait AccessibilityAdapter {
    fn update(&mut self, nodes: &[Node], focus: Option<NodeId>) -> Result<()>;
    fn announce(&mut self, message: &str, assertive: bool) -> Result<()>;
}

pub struct UnavailableAccessibility;

impl AccessibilityAdapter for UnavailableAccessibility {
    fn update(&mut self, _: &[Node], _: Option<NodeId>) -> Result<()> {
        Err(unavailable())
    }

    fn announce(&mut self, _: &str, _: bool) -> Result<()> {
        Err(unavailable())
    }
}

fn unavailable() -> ServiceError {
    ServiceError::Unsupported("Published GPUI 0.2.2 has no integrated native accessibility tree. An OS bridge and platform screen-reader validation are required.".into())
}
