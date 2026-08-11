use std::collections::BTreeSet;

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::NSView;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSRect, NSSize};

use crate::layout::{LayoutError, LayoutMeasurements, LayoutRect, LayoutSize, LayoutSpec};

#[derive(Debug)]
pub enum LayoutMountError {
    DuplicateBinding(String),
    MissingBinding(String),
    UnknownBinding(String),
    InvalidMeasurement { key: String, size: LayoutSize },
    Layout(LayoutError),
}

impl From<LayoutError> for LayoutMountError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

use super::support::rect;

pub struct LayoutSubview {
    pub key: String,
    pub view: Retained<NSView>,
}

impl LayoutSubview {
    pub fn new(key: impl Into<String>, view: Retained<NSView>) -> Self {
        Self {
            key: key.into(),
            view,
        }
    }
}

pub struct LayoutHostIvars {
    spec: LayoutSpec,
    subviews: Vec<LayoutSubview>,
}

define_class! {
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "HyprOverlayLayoutHostView"]
    #[ivars = LayoutHostIvars]
    pub struct LayoutHostView;

    impl LayoutHostView {
        #[unsafe(method(layout))]
        fn layout(&self) {
            let _: () = unsafe { msg_send![super(self), layout] };
            self.apply_layout();
        }
    }

    unsafe impl NSObjectProtocol for LayoutHostView {}
}

impl LayoutHostView {
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        spec: LayoutSpec,
        subviews: Vec<LayoutSubview>,
    ) -> Result<Retained<Self>, LayoutMountError> {
        validate_bindings(&spec, &subviews)?;
        let measurements = measurements(&subviews)?;
        spec.solve(layout_size(frame.size), &measurements)?;

        let this = Self::alloc(mtm).set_ivars(LayoutHostIvars { spec, subviews });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setAutoresizesSubviews(false);
        for subview in &this.ivars().subviews {
            this.addSubview(&subview.view);
        }
        this.apply_layout();
        Ok(this)
    }

    pub fn natural_size(
        spec: &LayoutSpec,
        subviews: &[LayoutSubview],
    ) -> Result<LayoutSize, LayoutMountError> {
        validate_bindings(spec, subviews)?;
        Ok(spec.natural_size(&measurements(subviews)?)?)
    }

    fn apply_layout(&self) {
        let Ok(measurements) = measurements(&self.ivars().subviews) else {
            return;
        };
        let bounds = self.bounds();
        let Ok(result) = self
            .ivars()
            .spec
            .solve(layout_size(bounds.size), &measurements)
        else {
            return;
        };

        for subview in &self.ivars().subviews {
            if let Some(frame) = result.frame(&subview.key) {
                subview
                    .view
                    .setFrame(top_left_to_appkit(frame, bounds.size.height));
            }
        }
    }
}

pub fn top_left_to_appkit(frame: LayoutRect, container_height: f64) -> NSRect {
    rect(
        frame.x,
        container_height - frame.y - frame.height,
        frame.width,
        frame.height,
    )
}

fn layout_size(size: NSSize) -> LayoutSize {
    LayoutSize::new(size.width, size.height)
}

fn validate_bindings(
    spec: &LayoutSpec,
    subviews: &[LayoutSubview],
) -> Result<(), LayoutMountError> {
    let expected: BTreeSet<_> = spec.leaf_keys().into_iter().collect();
    let mut actual = BTreeSet::new();
    for subview in subviews {
        if !actual.insert(subview.key.clone()) {
            return Err(LayoutMountError::DuplicateBinding(subview.key.clone()));
        }
        if !expected.contains(&subview.key) {
            return Err(LayoutMountError::UnknownBinding(subview.key.clone()));
        }
    }
    if let Some(key) = expected.difference(&actual).next() {
        return Err(LayoutMountError::MissingBinding(key.clone()));
    }
    Ok(())
}

fn measurements(subviews: &[LayoutSubview]) -> Result<LayoutMeasurements, LayoutMountError> {
    let mut measurements = LayoutMeasurements::new();
    for subview in subviews {
        let fitting = subview.view.fittingSize();
        let frame = subview.view.frame().size;
        let width = usable_extent(fitting.width).unwrap_or_else(|| frame.width.max(0.0));
        let height = usable_extent(fitting.height).unwrap_or_else(|| frame.height.max(0.0));
        let size = LayoutSize::new(width, height);
        if !size.is_valid() {
            return Err(LayoutMountError::InvalidMeasurement {
                key: subview.key.clone(),
                size,
            });
        }
        measurements.insert(subview.key.clone(), size);
    }
    Ok(measurements)
}

fn usable_extent(value: f64) -> Option<f64> {
    (value.is_finite() && value >= 0.0).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Dimension, LayoutNode};

    #[test]
    fn flips_top_left_frames_into_appkit_coordinates() {
        assert_eq!(
            top_left_to_appkit(LayoutRect::new(10.0, 12.0, 40.0, 18.0), 100.0),
            rect(10.0, 70.0, 40.0, 18.0)
        );
    }

    #[test]
    fn binding_key_validation_is_deterministic() {
        let spec = LayoutNode::leaf("known", Dimension::fixed(10.0), Dimension::fixed(10.0));
        let expected: BTreeSet<_> = spec.leaf_keys().into_iter().collect();
        assert_eq!(expected, BTreeSet::from(["known".to_string()]));
    }
}
