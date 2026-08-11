//! Small, deterministic layout primitives for native overlay chrome.
//!
//! This intentionally implements only what the overlays use: named leaves,
//! rows/columns, fixed or flexible gaps, and anchored children.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutPoint {
    pub x: f64,
    pub y: f64,
}

impl LayoutPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutSize {
    pub width: f64,
    pub height: f64,
}

impl LayoutSize {
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LayoutRect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_size(size: LayoutSize) -> Self {
        Self::new(0.0, 0.0, size.width, size.height)
    }

    pub const fn size(self) -> LayoutSize {
        LayoutSize::new(self.width, self.height)
    }

    pub fn is_valid(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.size().is_valid()
    }

    pub fn inset(self, insets: Insets) -> Self {
        Self::new(
            self.x + insets.left,
            self.y + insets.top,
            (self.width - insets.left - insets.right).max(0.0),
            (self.height - insets.top - insets.bottom).max(0.0),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Insets {
    pub const ZERO: Self = Self::all(0.0);

    pub const fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn all(value: f64) -> Self {
        Self::new(value, value, value, value)
    }

    pub const fn xy(horizontal: f64, vertical: f64) -> Self {
        Self::new(vertical, horizontal, vertical, horizontal)
    }

    pub(crate) fn is_valid(self) -> bool {
        [self.top, self.right, self.bottom, self.left]
            .into_iter()
            .all(|value| value.is_finite() && value >= 0.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dimension {
    Fixed(f64),
    Intrinsic { min: f64 },
    Fill { min: f64, weight: f64 },
}

impl Dimension {
    pub const fn fixed(value: f64) -> Self {
        Self::Fixed(value)
    }

    pub const fn intrinsic() -> Self {
        Self::Intrinsic { min: 0.0 }
    }

    pub const fn fill(weight: f64) -> Self {
        Self::Fill { min: 0.0, weight }
    }

    pub const fn fill_min(min: f64, weight: f64) -> Self {
        Self::Fill { min, weight }
    }

    fn clamp(self, intrinsic: f64) -> f64 {
        match self {
            Self::Fixed(value) => value,
            Self::Intrinsic { min } => intrinsic.max(min),
            Self::Fill { min, .. } => min,
        }
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Fixed(value) => value.is_finite() && value >= 0.0,
            Self::Intrinsic { min } => min.is_finite() && min >= 0.0,
            Self::Fill { min, weight } => {
                min.is_finite() && min >= 0.0 && weight.is_finite() && weight > 0.0
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Anchor {
    Start(f64),
    Center(f64),
    End(f64),
    Stretch { start: f64, end: f64 },
}

impl Anchor {
    fn is_valid(self) -> bool {
        match self {
            Self::Start(value) | Self::Center(value) | Self::End(value) => value.is_finite(),
            Self::Stretch { start, end } => start.is_finite() && end.is_finite(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct OverlayChild {
    node: LayoutNode,
    horizontal: Anchor,
    vertical: Anchor,
}

impl OverlayChild {
    pub fn new(node: LayoutNode, horizontal: Anchor, vertical: Anchor) -> Self {
        Self {
            node,
            horizontal,
            vertical,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Row,
    Column,
}

#[derive(Clone, Debug)]
enum LayoutKind {
    Leaf {
        key: String,
        width: Dimension,
        height: Dimension,
    },
    Gap(Dimension),
    Stack {
        direction: Direction,
        children: Vec<LayoutNode>,
        padding: Insets,
        gap: f64,
        align: Alignment,
    },
    Overlay(Vec<OverlayChild>),
}

#[derive(Clone, Debug)]
pub struct LayoutNode(LayoutKind);

pub type LayoutSpec = LayoutNode;

impl LayoutNode {
    pub fn leaf(key: impl Into<String>, width: Dimension, height: Dimension) -> Self {
        Self(LayoutKind::Leaf {
            key: key.into(),
            width,
            height,
        })
    }

    pub fn fixed_leaf(key: impl Into<String>, width: f64, height: f64) -> Self {
        Self::leaf(key, Dimension::fixed(width), Dimension::fixed(height))
    }

    pub fn intrinsic_leaf(key: impl Into<String>) -> Self {
        Self::leaf(key, Dimension::intrinsic(), Dimension::intrinsic())
    }

    pub fn space(width: f64) -> Self {
        Self(LayoutKind::Gap(Dimension::fixed(width)))
    }

    pub fn flex(min: f64) -> Self {
        Self(LayoutKind::Gap(Dimension::fill_min(min, 1.0)))
    }

    pub fn row(children: Vec<Self>) -> Self {
        Self::stack(Direction::Row, children)
    }

    pub fn column(children: Vec<Self>) -> Self {
        Self::stack(Direction::Column, children)
    }

    fn stack(direction: Direction, children: Vec<Self>) -> Self {
        Self(LayoutKind::Stack {
            direction,
            children,
            padding: Insets::ZERO,
            gap: 0.0,
            align: Alignment::Start,
        })
    }

    pub fn overlay(children: Vec<OverlayChild>) -> Self {
        Self(LayoutKind::Overlay(children))
    }

    pub fn padding(mut self, value: Insets) -> Self {
        if let LayoutKind::Stack { padding, .. } = &mut self.0 {
            *padding = value;
        }
        self
    }

    pub fn gap(mut self, value: f64) -> Self {
        if let LayoutKind::Stack { gap, .. } = &mut self.0 {
            *gap = value;
        }
        self
    }

    pub fn cross_alignment(mut self, value: Alignment) -> Self {
        if let LayoutKind::Stack { align, .. } = &mut self.0 {
            *align = value;
        }
        self
    }

    pub fn natural_size(
        &self,
        measurements: &LayoutMeasurements,
    ) -> Result<LayoutSize, LayoutError> {
        self.validate(measurements)?;
        Ok(natural_size(self, measurements))
    }

    pub fn solve(
        &self,
        container: LayoutSize,
        measurements: &LayoutMeasurements,
    ) -> Result<LayoutResult, LayoutError> {
        if !container.is_valid() {
            return Err(LayoutError::InvalidGeometry);
        }
        self.validate(measurements)?;
        let mut frames = BTreeMap::new();
        layout_node(
            self,
            LayoutRect::from_size(container),
            measurements,
            &mut frames,
        );
        Ok(LayoutResult {
            size: container,
            frames,
        })
    }

    pub fn leaf_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        collect_keys(self, &mut keys);
        keys
    }

    fn validate(&self, measurements: &LayoutMeasurements) -> Result<(), LayoutError> {
        let mut seen = BTreeSet::new();
        validate_node(self, measurements, &mut seen)
    }
}

#[derive(Clone, Debug, Default)]
pub struct LayoutMeasurements(BTreeMap<String, LayoutSize>);

impl LayoutMeasurements {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, key: impl Into<String>, size: LayoutSize) -> Self {
        self.insert(key, size);
        self
    }

    pub fn insert(&mut self, key: impl Into<String>, size: LayoutSize) {
        self.0.insert(key.into(), size);
    }

    fn get(&self, key: &str) -> Option<LayoutSize> {
        self.0.get(key).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutResult {
    pub size: LayoutSize,
    frames: BTreeMap<String, LayoutRect>,
}

impl LayoutResult {
    pub fn frame(&self, key: &str) -> Option<LayoutRect> {
        self.frames.get(key).copied()
    }

    pub fn frames(&self) -> impl Iterator<Item = (&str, LayoutRect)> {
        self.frames
            .iter()
            .map(|(key, frame)| (key.as_str(), *frame))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    InvalidGeometry,
    DuplicateKey(String),
    MissingMeasurement(String),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGeometry => f.write_str("invalid layout geometry"),
            Self::DuplicateKey(key) => write!(f, "duplicate layout key `{key}`"),
            Self::MissingMeasurement(key) => write!(f, "missing layout measurement for `{key}`"),
        }
    }
}

impl std::error::Error for LayoutError {}

fn validate_node(
    node: &LayoutNode,
    measurements: &LayoutMeasurements,
    seen: &mut BTreeSet<String>,
) -> Result<(), LayoutError> {
    match &node.0 {
        LayoutKind::Leaf { key, width, height } => {
            if key.is_empty() || !width.is_valid() || !height.is_valid() {
                return Err(LayoutError::InvalidGeometry);
            }
            if !seen.insert(key.clone()) {
                return Err(LayoutError::DuplicateKey(key.clone()));
            }
            let needs_measurement = matches!(width, Dimension::Intrinsic { .. })
                || matches!(height, Dimension::Intrinsic { .. });
            if needs_measurement {
                let size = measurements
                    .get(key)
                    .ok_or_else(|| LayoutError::MissingMeasurement(key.clone()))?;
                if !size.is_valid() {
                    return Err(LayoutError::InvalidGeometry);
                }
            }
        }
        LayoutKind::Gap(size) if !size.is_valid() => return Err(LayoutError::InvalidGeometry),
        LayoutKind::Gap(_) => {}
        LayoutKind::Stack {
            children,
            padding,
            gap,
            ..
        } => {
            if !padding.is_valid() || !gap.is_finite() || *gap < 0.0 {
                return Err(LayoutError::InvalidGeometry);
            }
            for child in children {
                validate_node(child, measurements, seen)?;
            }
        }
        LayoutKind::Overlay(children) => {
            for child in children {
                if !child.horizontal.is_valid() || !child.vertical.is_valid() {
                    return Err(LayoutError::InvalidGeometry);
                }
                validate_node(&child.node, measurements, seen)?;
            }
        }
    }
    Ok(())
}

fn natural_size(node: &LayoutNode, measurements: &LayoutMeasurements) -> LayoutSize {
    match &node.0 {
        LayoutKind::Leaf { key, width, height } => {
            let intrinsic = measurements.get(key).unwrap_or_default();
            LayoutSize::new(width.clamp(intrinsic.width), height.clamp(intrinsic.height))
        }
        LayoutKind::Gap(size) => LayoutSize::new(size.clamp(0.0), 0.0),
        LayoutKind::Stack {
            direction,
            children,
            padding,
            gap,
            ..
        } => {
            let sizes: Vec<_> = children
                .iter()
                .map(|child| natural_size(child, measurements))
                .collect();
            let gaps = gap * children.len().saturating_sub(1) as f64;
            match direction {
                Direction::Row => LayoutSize::new(
                    padding.left
                        + padding.right
                        + sizes.iter().map(|size| size.width).sum::<f64>()
                        + gaps,
                    padding.top
                        + padding.bottom
                        + sizes.iter().map(|size| size.height).fold(0.0, f64::max),
                ),
                Direction::Column => LayoutSize::new(
                    padding.left
                        + padding.right
                        + sizes.iter().map(|size| size.width).fold(0.0, f64::max),
                    padding.top
                        + padding.bottom
                        + sizes.iter().map(|size| size.height).sum::<f64>()
                        + gaps,
                ),
            }
        }
        LayoutKind::Overlay(children) => {
            children
                .iter()
                .fold(LayoutSize::default(), |result, child| {
                    let size = natural_size(&child.node, measurements);
                    LayoutSize::new(
                        result
                            .width
                            .max(anchor_natural(child.horizontal, size.width)),
                        result
                            .height
                            .max(anchor_natural(child.vertical, size.height)),
                    )
                })
        }
    }
}

fn anchor_natural(anchor: Anchor, extent: f64) -> f64 {
    match anchor {
        Anchor::Start(offset) | Anchor::End(offset) => offset.max(0.0) + extent,
        Anchor::Center(offset) => extent + offset.abs() * 2.0,
        Anchor::Stretch { start, end } => start.max(0.0) + extent + end.max(0.0),
    }
}

fn layout_node(
    node: &LayoutNode,
    frame: LayoutRect,
    measurements: &LayoutMeasurements,
    frames: &mut BTreeMap<String, LayoutRect>,
) {
    match &node.0 {
        LayoutKind::Leaf { key, .. } => {
            frames.insert(key.clone(), frame);
        }
        LayoutKind::Gap(_) => {}
        LayoutKind::Stack {
            direction,
            children,
            padding,
            gap,
            align,
        } => {
            layout_stack(
                *direction,
                children,
                *padding,
                *gap,
                *align,
                frame,
                measurements,
                frames,
            );
        }
        LayoutKind::Overlay(children) => {
            for child in children {
                let natural = natural_size(&child.node, measurements);
                let (x, width) =
                    anchored_axis(frame.x, frame.width, natural.width, child.horizontal);
                let (y, height) =
                    anchored_axis(frame.y, frame.height, natural.height, child.vertical);
                layout_node(
                    &child.node,
                    LayoutRect::new(x, y, width, height),
                    measurements,
                    frames,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn layout_stack(
    direction: Direction,
    children: &[LayoutNode],
    padding: Insets,
    gap: f64,
    align: Alignment,
    frame: LayoutRect,
    measurements: &LayoutMeasurements,
    frames: &mut BTreeMap<String, LayoutRect>,
) {
    let horizontal = direction == Direction::Row;
    let available_main = if horizontal {
        frame.width - padding.left - padding.right
    } else {
        frame.height - padding.top - padding.bottom
    }
    .max(0.0);
    let available_cross = if horizontal {
        frame.height - padding.top - padding.bottom
    } else {
        frame.width - padding.left - padding.right
    }
    .max(0.0);

    let natural: Vec<_> = children
        .iter()
        .map(|child| natural_size(child, measurements))
        .collect();
    let mut main: Vec<_> = children
        .iter()
        .zip(&natural)
        .map(|(child, size)| {
            main_dimension(child, horizontal).clamp(if horizontal {
                size.width
            } else {
                size.height
            })
        })
        .collect();
    let gap_total = gap * children.len().saturating_sub(1) as f64;
    distribute_space(
        children,
        horizontal,
        (available_main - gap_total).max(0.0),
        &mut main,
    );

    let mut cursor = if horizontal {
        frame.x + padding.left
    } else {
        frame.y + padding.top
    };
    for ((child, natural), main_extent) in children.iter().zip(natural).zip(main) {
        let natural_cross = if horizontal {
            natural.height
        } else {
            natural.width
        };
        let cross_dimension = main_dimension(child, !horizontal);
        let cross_extent = match align {
            Alignment::Stretch => available_cross,
            _ => cross_dimension.clamp(natural_cross).min(available_cross),
        };
        let cross_offset = match align {
            Alignment::Start | Alignment::Stretch => 0.0,
            Alignment::Center => (available_cross - cross_extent) / 2.0,
            Alignment::End => available_cross - cross_extent,
        };
        let child_frame = if horizontal {
            LayoutRect::new(
                cursor,
                frame.y + padding.top + cross_offset,
                main_extent,
                cross_extent,
            )
        } else {
            LayoutRect::new(
                frame.x + padding.left + cross_offset,
                cursor,
                cross_extent,
                main_extent,
            )
        };
        layout_node(child, child_frame, measurements, frames);
        cursor += main_extent + gap;
    }
}

fn main_dimension(node: &LayoutNode, horizontal: bool) -> Dimension {
    match &node.0 {
        LayoutKind::Leaf { width, height, .. } => {
            if horizontal {
                *width
            } else {
                *height
            }
        }
        LayoutKind::Gap(size) => *size,
        _ => Dimension::Intrinsic { min: 0.0 },
    }
}

fn distribute_space(children: &[LayoutNode], horizontal: bool, available: f64, sizes: &mut [f64]) {
    let used: f64 = sizes.iter().sum();
    if available > used {
        let fill_weight: f64 = children
            .iter()
            .filter_map(|child| match main_dimension(child, horizontal) {
                Dimension::Fill { weight, .. } => Some(weight),
                _ => None,
            })
            .sum();
        if fill_weight > 0.0 {
            let extra = available - used;
            for (child, size) in children.iter().zip(sizes) {
                if let Dimension::Fill { weight, .. } = main_dimension(child, horizontal) {
                    *size += extra * weight / fill_weight;
                }
            }
        }
    } else if used > available {
        let shrinkable: f64 = children
            .iter()
            .zip(sizes.iter())
            .filter_map(|(child, size)| match main_dimension(child, horizontal) {
                Dimension::Intrinsic { min, .. } => Some((*size - min).max(0.0)),
                _ => None,
            })
            .sum();
        let shortage = (used - available).min(shrinkable);
        if shrinkable > 0.0 {
            for (child, size) in children.iter().zip(sizes) {
                if let Dimension::Intrinsic { min, .. } = main_dimension(child, horizontal) {
                    *size -= shortage * (*size - min).max(0.0) / shrinkable;
                }
            }
        }
    }
}

fn anchored_axis(origin: f64, available: f64, natural: f64, anchor: Anchor) -> (f64, f64) {
    match anchor {
        Anchor::Start(offset) => (origin + offset, natural),
        Anchor::Center(offset) => (origin + (available - natural) / 2.0 + offset, natural),
        Anchor::End(offset) => (origin + available - natural - offset, natural),
        Anchor::Stretch { start, end } => (origin + start, (available - start - end).max(0.0)),
    }
}

fn collect_keys(node: &LayoutNode, keys: &mut Vec<String>) {
    match &node.0 {
        LayoutKind::Leaf { key, .. } => keys.push(key.clone()),
        LayoutKind::Gap(_) => {}
        LayoutKind::Stack { children, .. } => {
            children.iter().for_each(|child| collect_keys(child, keys))
        }
        LayoutKind::Overlay(children) => children
            .iter()
            .for_each(|child| collect_keys(&child.node, keys)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_allocates_remaining_width_and_shrinks_intrinsic_content() {
        let spec = LayoutNode::row(vec![
            LayoutNode::fixed_leaf("icon", 20.0, 20.0),
            LayoutNode::space(6.0),
            LayoutNode::leaf(
                "label",
                Dimension::Intrinsic { min: 1.0 },
                Dimension::intrinsic(),
            ),
            LayoutNode::flex(8.0),
            LayoutNode::fixed_leaf("close", 20.0, 20.0),
        ])
        .padding(Insets::xy(10.0, 4.0))
        .cross_alignment(Alignment::Center);
        let measurements = LayoutMeasurements::new().with("label", LayoutSize::new(100.0, 16.0));

        let wide = spec
            .solve(LayoutSize::new(200.0, 36.0), &measurements)
            .unwrap();
        assert_eq!(wide.frame("close").unwrap().x, 170.0);
        let narrow = spec
            .solve(LayoutSize::new(100.0, 36.0), &measurements)
            .unwrap();
        assert_eq!(narrow.frame("label").unwrap().width, 26.0);
    }

    #[test]
    fn overlay_anchors_and_stretches_named_children() {
        let spec = LayoutNode::overlay(vec![
            OverlayChild::new(
                LayoutNode::fixed_leaf("close", 20.0, 20.0),
                Anchor::End(8.0),
                Anchor::Start(6.0),
            ),
            OverlayChild::new(
                LayoutNode::fixed_leaf("row", 1.0, 18.0),
                Anchor::Stretch {
                    start: 10.0,
                    end: 10.0,
                },
                Anchor::End(5.0),
            ),
        ]);
        let result = spec
            .solve(LayoutSize::new(100.0, 60.0), &LayoutMeasurements::new())
            .unwrap();
        assert_eq!(
            result.frame("close"),
            Some(LayoutRect::new(72.0, 6.0, 20.0, 20.0))
        );
        assert_eq!(
            result.frame("row"),
            Some(LayoutRect::new(10.0, 37.0, 80.0, 18.0))
        );
    }

    #[test]
    fn invalid_and_duplicate_layouts_fail() {
        let duplicate = LayoutNode::row(vec![
            LayoutNode::fixed_leaf("same", 1.0, 1.0),
            LayoutNode::fixed_leaf("same", 1.0, 1.0),
        ]);
        assert!(matches!(
            duplicate.solve(LayoutSize::new(10.0, 10.0), &LayoutMeasurements::new()),
            Err(LayoutError::DuplicateKey(_))
        ));
        assert!(
            LayoutNode::fixed_leaf("bad", f64::NAN, 1.0)
                .solve(LayoutSize::new(10.0, 10.0), &LayoutMeasurements::new())
                .is_err()
        );
    }
}
