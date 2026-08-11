//! Platform-neutral vector path descriptions used by native overlay renderers.

use crate::layout::{LayoutPoint, LayoutRect, LayoutSize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathError {
    InvalidViewBox,
    InvalidBounds,
    InvalidCommand,
    UnsupportedSvgCommand(char),
    InvalidSvgData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathWinding {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathFit {
    Stretch,
    AspectFit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCommand {
    MoveTo(LayoutPoint),
    LineTo(LayoutPoint),
    CubicTo {
        control1: LayoutPoint,
        control2: LayoutPoint,
        to: LayoutPoint,
    },
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathSpec {
    pub view_box: LayoutSize,
    pub winding: PathWinding,
    pub commands: Vec<PathCommand>,
}

impl PathSpec {
    pub fn new(
        view_box: LayoutSize,
        winding: PathWinding,
        commands: Vec<PathCommand>,
    ) -> Result<Self, PathError> {
        let spec = Self {
            view_box,
            winding,
            commands,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn transform(&self, bounds: LayoutRect, fit: PathFit) -> Result<PathTransform, PathError> {
        self.validate()?;
        if !bounds.is_valid() {
            return Err(PathError::InvalidBounds);
        }

        let scale_x = bounds.width / self.view_box.width;
        let scale_y = bounds.height / self.view_box.height;
        let (scale_x, scale_y) = match fit {
            PathFit::Stretch => (scale_x, scale_y),
            PathFit::AspectFit => {
                let scale = scale_x.min(scale_y);
                (scale, scale)
            }
        };
        let drawn_width = self.view_box.width * scale_x;
        let drawn_height = self.view_box.height * scale_y;

        Ok(PathTransform {
            x: bounds.x + (bounds.width - drawn_width) / 2.0,
            y: bounds.y + (bounds.height - drawn_height) / 2.0,
            scale_x,
            scale_y,
        })
    }

    fn validate(&self) -> Result<(), PathError> {
        if !self.view_box.is_valid() || self.view_box.width <= 0.0 || self.view_box.height <= 0.0 {
            return Err(PathError::InvalidViewBox);
        }
        if self.commands.iter().all(valid_command) {
            Ok(())
        } else {
            Err(PathError::InvalidCommand)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathTransform {
    x: f64,
    y: f64,
    scale_x: f64,
    scale_y: f64,
}

impl PathTransform {
    /// Maps a path point into top-left-origin layout coordinates.
    pub fn point(self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(
            self.x + point.x * self.scale_x,
            self.y + point.y * self.scale_y,
        )
    }
}

/// Parses the absolute SVG commands used by overlay artwork: `M`, `L`, `H`,
/// `V`, `C`, and `Z`. Relative and arc commands intentionally fail closed.
pub fn parse_svg_path(
    data: &str,
    view_box: LayoutSize,
    winding: PathWinding,
) -> Result<PathSpec, PathError> {
    let mut cursor = SvgPathCursor::new(data);
    let mut active_command = None;
    let mut current = LayoutPoint::default();
    let mut subpath_start = LayoutPoint::default();
    let mut commands = Vec::new();

    while !cursor.is_finished() {
        if let Some(command) = cursor.take_command() {
            if !matches!(command, b'M' | b'L' | b'H' | b'V' | b'C' | b'Z') {
                return Err(PathError::UnsupportedSvgCommand(command as char));
            }
            active_command = Some(command);
        }

        match active_command {
            Some(b'M') => {
                let point = cursor.take_point()?;
                commands.push(PathCommand::MoveTo(point));
                current = point;
                subpath_start = point;
                active_command = Some(b'L');
            }
            Some(b'L') => {
                let point = cursor.take_point()?;
                commands.push(PathCommand::LineTo(point));
                current = point;
            }
            Some(b'H') => {
                let x = cursor.take_number()?;
                current.x = x;
                commands.push(PathCommand::LineTo(current));
            }
            Some(b'V') => {
                let y = cursor.take_number()?;
                current.y = y;
                commands.push(PathCommand::LineTo(current));
            }
            Some(b'C') => {
                let control1 = cursor.take_point()?;
                let control2 = cursor.take_point()?;
                let to = cursor.take_point()?;
                commands.push(PathCommand::CubicTo {
                    control1,
                    control2,
                    to,
                });
                current = to;
            }
            Some(b'Z') => {
                commands.push(PathCommand::Close);
                current = subpath_start;
                active_command = None;
            }
            None => return Err(PathError::InvalidSvgData),
            Some(command) => return Err(PathError::UnsupportedSvgCommand(command as char)),
        }
    }

    if commands.is_empty() {
        return Err(PathError::InvalidSvgData);
    }
    PathSpec::new(view_box, winding, commands)
}

fn valid_command(command: &PathCommand) -> bool {
    match command {
        PathCommand::MoveTo(point) | PathCommand::LineTo(point) => valid_point(*point),
        PathCommand::CubicTo {
            control1,
            control2,
            to,
        } => valid_point(*control1) && valid_point(*control2) && valid_point(*to),
        PathCommand::Close => true,
    }
}

fn valid_point(point: LayoutPoint) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

struct SvgPathCursor<'a> {
    path: &'a [u8],
    index: usize,
}

impl<'a> SvgPathCursor<'a> {
    fn new(path_data: &'a str) -> Self {
        Self {
            path: path_data.as_bytes(),
            index: 0,
        }
    }

    fn is_finished(&mut self) -> bool {
        self.skip_separators();
        self.index >= self.path.len()
    }

    fn take_command(&mut self) -> Option<u8> {
        self.skip_separators();
        let byte = *self.path.get(self.index)?;
        byte.is_ascii_alphabetic().then(|| {
            self.index += 1;
            byte
        })
    }

    fn take_point(&mut self) -> Result<LayoutPoint, PathError> {
        Ok(LayoutPoint::new(self.take_number()?, self.take_number()?))
    }

    fn take_number(&mut self) -> Result<f64, PathError> {
        self.skip_separators();
        let start = self.index;

        if matches!(self.path.get(self.index), Some(b'+') | Some(b'-')) {
            self.index += 1;
        }

        let integer_start = self.index;
        while self
            .path
            .get(self.index)
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            self.index += 1;
        }

        if matches!(self.path.get(self.index), Some(b'.')) {
            self.index += 1;
            while self
                .path
                .get(self.index)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.index += 1;
            }
        }
        if self.index == integer_start {
            return Err(PathError::InvalidSvgData);
        }

        if matches!(self.path.get(self.index), Some(b'e') | Some(b'E')) {
            self.index += 1;
            if matches!(self.path.get(self.index), Some(b'+') | Some(b'-')) {
                self.index += 1;
            }
            let exponent_start = self.index;
            while self
                .path
                .get(self.index)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                self.index += 1;
            }
            if self.index == exponent_start {
                return Err(PathError::InvalidSvgData);
            }
        }

        std::str::from_utf8(&self.path[start..self.index])
            .ok()
            .and_then(|value| value.parse().ok())
            .ok_or(PathError::InvalidSvgData)
    }

    fn skip_separators(&mut self) {
        while self
            .path
            .get(self.index)
            .is_some_and(|byte| byte.is_ascii_whitespace() || *byte == b',')
        {
            self.index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_fit_centers_and_transforms_points() {
        let path = PathSpec::new(
            LayoutSize::new(100.0, 50.0),
            PathWinding::NonZero,
            vec![PathCommand::MoveTo(LayoutPoint::new(100.0, 50.0))],
        )
        .unwrap();
        let transform = path
            .transform(
                LayoutRect::new(10.0, 20.0, 200.0, 200.0),
                PathFit::AspectFit,
            )
            .unwrap();
        assert_eq!(
            transform.point(LayoutPoint::new(0.0, 0.0)),
            LayoutPoint::new(10.0, 70.0)
        );
        assert_eq!(
            transform.point(LayoutPoint::new(100.0, 50.0)),
            LayoutPoint::new(210.0, 170.0)
        );
    }

    #[test]
    fn svg_parser_supports_exported_overlay_commands() {
        let path = parse_svg_path(
            "M0 0H10V5L2 5C1 4 1 2 0 0ZM2 2L4 4Z",
            LayoutSize::new(10.0, 5.0),
            PathWinding::EvenOdd,
        )
        .unwrap();
        assert_eq!(path.commands.len(), 9);
        assert_eq!(path.winding, PathWinding::EvenOdd);
    }

    #[test]
    fn svg_parser_rejects_relative_and_malformed_data() {
        assert!(matches!(
            parse_svg_path("m0 0", LayoutSize::new(1.0, 1.0), PathWinding::NonZero,),
            Err(PathError::UnsupportedSvgCommand('m'))
        ));
        assert!(matches!(
            parse_svg_path("M0", LayoutSize::new(1.0, 1.0), PathWinding::NonZero,),
            Err(PathError::InvalidSvgData)
        ));
    }
}
