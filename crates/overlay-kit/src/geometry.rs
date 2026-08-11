/// Error for fallible conversions into [`CheckedRect`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidRect;

impl std::fmt::Display for InvalidRect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid rectangle geometry")
    }
}

impl std::error::Error for InvalidRect {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CheckedRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    right: f64,
    bottom: f64,
}

impl CheckedRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Option<Self> {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }

        let right = x + width;
        let bottom = y + height;
        if !right.is_finite() || !bottom.is_finite() {
            return None;
        }

        Some(Self {
            x,
            y,
            width,
            height,
            right,
            bottom,
        })
    }

    pub fn x(self) -> f64 {
        self.x
    }

    pub fn y(self) -> f64 {
        self.y
    }

    pub fn width(self) -> f64 {
        self.width
    }

    pub fn height(self) -> f64 {
        self.height
    }

    pub fn right(self) -> f64 {
        self.right
    }

    pub fn bottom(self) -> f64 {
        self.bottom
    }

    pub fn area(self) -> f64 {
        self.width * self.height
    }

    pub fn encloses(self, inner: Self, tolerance: f64) -> bool {
        if !tolerance.is_finite() || tolerance < 0.0 {
            return false;
        }
        let left = self.x - tolerance;
        let top = self.y - tolerance;
        let right = self.right + tolerance;
        let bottom = self.bottom + tolerance;
        left.is_finite()
            && top.is_finite()
            && right.is_finite()
            && bottom.is_finite()
            && inner.x >= left
            && inner.y >= top
            && inner.right <= right
            && inner.bottom <= bottom
    }
}

#[cfg(test)]
mod tests {
    use super::CheckedRect;

    #[test]
    fn rejects_non_finite_components() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(CheckedRect::new(value, 0.0, 1.0, 1.0).is_none());
            assert!(CheckedRect::new(0.0, value, 1.0, 1.0).is_none());
            assert!(CheckedRect::new(0.0, 0.0, value, 1.0).is_none());
            assert!(CheckedRect::new(0.0, 0.0, 1.0, value).is_none());
        }
    }

    #[test]
    fn rejects_non_positive_extents_and_overflowing_edges() {
        assert!(CheckedRect::new(0.0, 0.0, 0.0, 1.0).is_none());
        assert!(CheckedRect::new(0.0, 0.0, 1.0, 0.0).is_none());
        assert!(CheckedRect::new(0.0, 0.0, -1.0, 1.0).is_none());
        assert!(CheckedRect::new(0.0, 0.0, 1.0, -1.0).is_none());
        assert!(CheckedRect::new(f64::MAX, 0.0, f64::MAX, 1.0).is_none());
        assert!(CheckedRect::new(0.0, f64::MAX, 1.0, f64::MAX).is_none());
    }

    #[test]
    fn accepts_negative_origins_and_tiny_positive_rectangles() {
        let rect =
            CheckedRect::new(-2560.0, -720.0, f64::MIN_POSITIVE, 0.25).expect("valid rectangle");
        assert_eq!(rect.x(), -2560.0);
        assert_eq!(rect.y(), -720.0);
        assert!(rect.width() > 0.0);
        assert_eq!(rect.height(), 0.25);
        assert!(rect.right().is_finite());
        assert!(rect.bottom().is_finite());
    }

    #[test]
    fn computes_area_and_tolerance_aware_enclosure() {
        let outer = CheckedRect::new(-10.0, 20.0, 100.0, 50.0).expect("outer rectangle");
        let inner = CheckedRect::new(-11.0, 21.0, 102.0, 48.0).expect("inner rectangle");

        assert_eq!(outer.area(), 5_000.0);
        assert!(!outer.encloses(inner, 0.0));
        assert!(outer.encloses(inner, 1.0));
        assert!(!outer.encloses(inner, -1.0));
        assert!(!outer.encloses(inner, f64::NAN));
    }
}
