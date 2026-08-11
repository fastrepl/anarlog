//! Coordinate-system-neutral rectangle placement for overlay panels.

use crate::layout::{Alignment, Insets, LayoutRect, LayoutSize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    InvalidSpec,
    InvalidGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlacementExtent {
    Fixed(f64),
    Intrinsic { min: f64, max: Option<f64> },
    Fill { min: f64, max: Option<f64> },
}

impl PlacementExtent {
    pub const fn fixed(value: f64) -> Self {
        Self::Fixed(value)
    }

    pub const fn intrinsic() -> Self {
        Self::Intrinsic {
            min: 0.0,
            max: None,
        }
    }

    pub const fn intrinsic_bounded(min: f64, max: f64) -> Self {
        Self::Intrinsic {
            min,
            max: Some(max),
        }
    }

    pub const fn fill_bounded(min: f64, max: f64) -> Self {
        Self::Fill {
            min,
            max: Some(max),
        }
    }

    fn resolve(self, available: f64, intrinsic: f64) -> Option<f64> {
        let value = match self {
            Self::Fixed(value) => value,
            Self::Intrinsic { min, max } => clamp(intrinsic, min, max)?,
            Self::Fill { min, max } => clamp(available, min, max)?,
        };
        finite_non_negative(value).then_some(value)
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Fixed(value) => finite_non_negative(value),
            Self::Intrinsic { min, max } | Self::Fill { min, max } => valid_bounds(min, max),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacementSpec {
    pub width: PlacementExtent,
    pub height: PlacementExtent,
    pub horizontal: Alignment,
    pub vertical: Alignment,
    /// Insets applied to the preferred placement region.
    pub margins: Insets,
    /// Insets applied to the visible region before the final clamp.
    pub safe_insets: Insets,
}

impl PlacementSpec {
    pub const fn new(width: PlacementExtent, height: PlacementExtent) -> Self {
        Self {
            width,
            height,
            horizontal: Alignment::Start,
            vertical: Alignment::Start,
            margins: Insets::ZERO,
            safe_insets: Insets::ZERO,
        }
    }

    pub const fn align(mut self, horizontal: Alignment, vertical: Alignment) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }

    pub const fn margins(mut self, margins: Insets) -> Self {
        self.margins = margins;
        self
    }

    pub const fn safe_insets(mut self, safe_insets: Insets) -> Self {
        self.safe_insets = safe_insets;
        self
    }

    pub fn resolve(
        self,
        preferred_region: LayoutRect,
        visible_region: LayoutRect,
        intrinsic: LayoutSize,
    ) -> Result<LayoutRect, PlacementError> {
        if !self.width.is_valid()
            || !self.height.is_valid()
            || self.horizontal == Alignment::Stretch
            || self.vertical == Alignment::Stretch
            || !self.margins.is_valid()
            || !self.safe_insets.is_valid()
        {
            return Err(PlacementError::InvalidSpec);
        }
        if !preferred_region.is_valid() || !visible_region.is_valid() || !intrinsic.is_valid() {
            return Err(PlacementError::InvalidGeometry);
        }

        let preferred = preferred_region.inset(self.margins);
        let safe = visible_region.inset(self.safe_insets);
        let width = self
            .width
            .resolve(preferred.width, intrinsic.width)
            .ok_or(PlacementError::InvalidSpec)?;
        let height = self
            .height
            .resolve(preferred.height, intrinsic.height)
            .ok_or(PlacementError::InvalidSpec)?;

        let preferred_x = aligned_origin(preferred.x, preferred.width, width, self.horizontal);
        let preferred_y = aligned_origin(preferred.y, preferred.height, height, self.vertical);
        let x = clamp_origin(preferred_x, safe.x, safe.width, width);
        let y = clamp_origin(preferred_y, safe.y, safe.height, height);
        let result = LayoutRect::new(x, y, width, height);
        result
            .is_valid()
            .then_some(result)
            .ok_or(PlacementError::InvalidGeometry)
    }
}

fn aligned_origin(origin: f64, available: f64, extent: f64, alignment: Alignment) -> f64 {
    match alignment {
        Alignment::Start | Alignment::Stretch => origin,
        Alignment::Center => origin + (available - extent) / 2.0,
        Alignment::End => origin + available - extent,
    }
}

fn clamp_origin(preferred: f64, origin: f64, available: f64, extent: f64) -> f64 {
    let maximum = origin + available - extent;
    if origin <= maximum {
        preferred.clamp(origin, maximum)
    } else {
        origin + (available - extent) / 2.0
    }
}

fn clamp(value: f64, min: f64, max: Option<f64>) -> Option<f64> {
    if !valid_bounds(min, max) || !value.is_finite() {
        return None;
    }
    Some(max.map_or(value.max(min), |max| value.clamp(min, max)))
}

fn valid_bounds(min: f64, max: Option<f64>) -> bool {
    finite_non_negative(min) && max.is_none_or(|max| max.is_finite() && max >= min)
}

fn finite_non_negative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_intrinsic_content_at_the_max_axis_edge() {
        let spec = PlacementSpec::new(
            PlacementExtent::intrinsic_bounded(100.0, 400.0),
            PlacementExtent::fixed(42.0),
        )
        .align(Alignment::Center, Alignment::End)
        .margins(Insets::new(0.0, 0.0, 30.0, 0.0))
        .safe_insets(Insets::all(8.0));
        let result = spec
            .resolve(
                LayoutRect::new(0.0, 0.0, 1440.0, 900.0),
                LayoutRect::new(0.0, 0.0, 1440.0, 860.0),
                LayoutSize::new(338.0, 42.0),
            )
            .unwrap();

        assert_eq!(result, LayoutRect::new(551.0, 810.0, 338.0, 42.0));
    }

    #[test]
    fn right_pins_fill_width_and_clamps_to_visible_region() {
        let spec = PlacementSpec::new(
            PlacementExtent::fill_bounded(340.0, 680.0),
            PlacementExtent::fixed(128.0),
        )
        .align(Alignment::End, Alignment::Start)
        .margins(Insets::new(9.0, 8.0, 0.0, 10.0))
        .safe_insets(Insets::all(8.0));
        let result = spec
            .resolve(
                LayoutRect::new(320.0, 100.0, 880.0, 700.0),
                LayoutRect::new(0.0, 0.0, 1440.0, 860.0),
                LayoutSize::new(0.0, 0.0),
            )
            .unwrap();

        assert_eq!(result, LayoutRect::new(512.0, 109.0, 680.0, 128.0));
    }

    #[test]
    fn oversized_content_is_centered_in_the_safe_region() {
        let spec = PlacementSpec::new(PlacementExtent::fixed(200.0), PlacementExtent::fixed(80.0))
            .align(Alignment::Center, Alignment::Center)
            .safe_insets(Insets::all(10.0));
        let result = spec
            .resolve(
                LayoutRect::new(0.0, 0.0, 100.0, 60.0),
                LayoutRect::new(0.0, 0.0, 100.0, 60.0),
                LayoutSize::default(),
            )
            .unwrap();

        assert_eq!(result, LayoutRect::new(-50.0, -10.0, 200.0, 80.0));
    }

    #[test]
    fn invalid_values_fail_closed() {
        let spec = PlacementSpec::new(
            PlacementExtent::fixed(f64::NAN),
            PlacementExtent::fixed(10.0),
        );
        assert_eq!(
            spec.resolve(
                LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                LayoutRect::new(0.0, 0.0, 100.0, 100.0),
                LayoutSize::default(),
            ),
            Err(PlacementError::InvalidSpec)
        );
    }
}
